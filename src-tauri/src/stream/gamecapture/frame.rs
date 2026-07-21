use std::ffi::c_void;

use windows::Win32::Foundation::{HANDLE, HMODULE};
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL_11_0};
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D, D3D11_BIND_FLAG,
    D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAPPED_SUBRESOURCE,
    D3D11_MAP_READ, D3D11_RESOURCE_MISC_FLAG, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC,
    D3D11_USAGE_STAGING,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT, DXGI_FORMAT_B8G8R8A8_TYPELESS, DXGI_FORMAT_B8G8R8A8_UNORM,
    DXGI_FORMAT_B8G8R8A8_UNORM_SRGB, DXGI_FORMAT_R10G10B10A2_TYPELESS,
    DXGI_FORMAT_R10G10B10A2_UNORM, DXGI_FORMAT_R8G8B8A8_TYPELESS, DXGI_FORMAT_R8G8B8A8_UNORM,
    DXGI_FORMAT_R8G8B8A8_UNORM_SRGB, DXGI_SAMPLE_DESC,
};

use crate::logging::{mlog, LogCat};

#[derive(Clone, Copy)]
enum PixelConv {
    Bgra,    // passthrough
    Rgba,    // swap R<->B
    Rgb10a2, // unpack 10-bit -> 8-bit BGRA
}

fn resolve_format(fmt: DXGI_FORMAT) -> Option<(DXGI_FORMAT, PixelConv)> {
    match fmt {
        DXGI_FORMAT_B8G8R8A8_UNORM
        | DXGI_FORMAT_B8G8R8A8_UNORM_SRGB
        | DXGI_FORMAT_B8G8R8A8_TYPELESS => Some((DXGI_FORMAT_B8G8R8A8_UNORM, PixelConv::Bgra)),
        DXGI_FORMAT_R8G8B8A8_UNORM
        | DXGI_FORMAT_R8G8B8A8_UNORM_SRGB
        | DXGI_FORMAT_R8G8B8A8_TYPELESS => Some((DXGI_FORMAT_R8G8B8A8_UNORM, PixelConv::Rgba)),
        DXGI_FORMAT_R10G10B10A2_UNORM | DXGI_FORMAT_R10G10B10A2_TYPELESS => {
            Some((DXGI_FORMAT_R10G10B10A2_UNORM, PixelConv::Rgb10a2))
        }
        _ => None,
    }
}

struct OpenedTexture {
    shared: ID3D11Texture2D,
    staging: ID3D11Texture2D,
    handle: u32,
    width: u32,
    height: u32,
    conv: PixelConv,
}

pub(crate) struct SharedTextureReader {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    tex: Option<OpenedTexture>,
}

fn create_device() -> Result<(ID3D11Device, ID3D11DeviceContext), String> {
    let mut device = None;
    let mut context = None;
    let levels = [D3D_FEATURE_LEVEL_11_0];
    unsafe {
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            Some(&levels),
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut context),
        )
        .map_err(|e| format!("D3D11CreateDevice: {e}"))?;
    }
    Ok((
        device.ok_or("D3D11CreateDevice: null device")?,
        context.ok_or("D3D11CreateDevice: null context")?,
    ))
}

impl SharedTextureReader {
    pub(crate) fn new() -> Result<Self, String> {
        let (device, context) = create_device()?;
        Ok(Self {
            device,
            context,
            tex: None,
        })
    }

    pub(crate) fn current_handle(&self) -> u32 {
        self.tex.as_ref().map(|t| t.handle).unwrap_or(0)
    }

    pub(crate) fn clear(&mut self) {
        self.tex = None;
    }

    pub(crate) fn open_texture(&mut self, tex_handle: u32) -> Result<(u32, u32), String> {
        self.tex = None;
        unsafe {
            let handle = HANDLE(tex_handle as usize as *mut c_void);
            let mut shared: Option<ID3D11Texture2D> = None;
            self.device
                .OpenSharedResource(handle, &mut shared)
                .map_err(|e| format!("OpenSharedResource({tex_handle:#x}): {e}"))?;
            let shared = shared.ok_or("OpenSharedResource: null texture")?;

            let mut desc = D3D11_TEXTURE2D_DESC::default();
            shared.GetDesc(&mut desc);

            let (staging_fmt, conv) = resolve_format(desc.Format)
                .ok_or_else(|| format!("unsupported shared-texture format {}", desc.Format.0))?;
            mlog!(
                LogCat::Stream,
                "[gc] shared texture format {} (staging {}) {}x{}",
                desc.Format.0,
                staging_fmt.0,
                desc.Width,
                desc.Height
            );

            let sdesc = D3D11_TEXTURE2D_DESC {
                Width: desc.Width,
                Height: desc.Height,
                MipLevels: 1,
                ArraySize: 1,
                Format: staging_fmt,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: D3D11_USAGE_STAGING,
                BindFlags: D3D11_BIND_FLAG(0).0 as u32,
                CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                MiscFlags: D3D11_RESOURCE_MISC_FLAG(0).0 as u32,
            };
            let mut staging: Option<ID3D11Texture2D> = None;
            self.device
                .CreateTexture2D(&sdesc, None, Some(&mut staging))
                .map_err(|e| format!("CreateTexture2D(staging): {e}"))?;
            let staging = staging.ok_or("CreateTexture2D: null staging")?;

            self.tex = Some(OpenedTexture {
                shared,
                staging,
                handle: tex_handle,
                width: desc.Width,
                height: desc.Height,
                conv,
            });
            Ok((desc.Width, desc.Height))
        }
    }

    pub(crate) fn read_into(
        &self,
        out: &mut [u8],
        target_w: u32,
        target_h: u32,
    ) -> Result<(), String> {
        let tex = self.tex.as_ref().ok_or("no texture open")?;
        let (tw, th) = (tex.width as usize, tex.height as usize);
        let (dw, dh) = (target_w as usize, target_h as usize);
        let dst_pitch = dw * 4;
        let copy_w = tw.min(dw);
        let copy_h = th.min(dh);
        let sx = (tw - copy_w) / 2;
        let sy = (th - copy_h) / 2;
        let dx = (dw - copy_w) / 2;
        let dy = (dh - copy_h) / 2;
        unsafe {
            self.context.CopyResource(&tex.staging, &tex.shared);
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            self.context
                .Map(&tex.staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
                .map_err(|e| format!("Map(staging): {e}"))?;
            if tw != dw || th != dh {
                out.iter_mut().for_each(|b| *b = 0); // letterbox bars
            }
            let src = mapped.pData as *const u8;
            let src_pitch = mapped.RowPitch as usize;
            let out_ptr = out.as_mut_ptr();
            for row in 0..copy_h {
                let s = src.add((sy + row) * src_pitch + sx * 4);
                let d = out_ptr.add((dy + row) * dst_pitch + dx * 4);
                match tex.conv {
                    PixelConv::Bgra => std::ptr::copy_nonoverlapping(s, d, copy_w * 4),
                    PixelConv::Rgba => {
                        for px in 0..copy_w {
                            let sp = s.add(px * 4);
                            let dp = d.add(px * 4);
                            *dp = *sp.add(2); // B <- R
                            *dp.add(1) = *sp.add(1); // G
                            *dp.add(2) = *sp; // R <- B
                            *dp.add(3) = *sp.add(3); // A
                        }
                    }
                    PixelConv::Rgb10a2 => {
                        for px in 0..copy_w {
                            let v = (s.add(px * 4) as *const u32).read_unaligned();
                            let dp = d.add(px * 4);
                            *dp = ((v >> 22) & 0xFF) as u8; // B (top 8 of 10)
                            *dp.add(1) = ((v >> 12) & 0xFF) as u8; // G
                            *dp.add(2) = ((v >> 2) & 0xFF) as u8; // R
                            *dp.add(3) = 0xFF; // A (2-bit alpha -> opaque)
                        }
                    }
                }
            }
            self.context.Unmap(&tex.staging, 0);
        }
        Ok(())
    }
}
