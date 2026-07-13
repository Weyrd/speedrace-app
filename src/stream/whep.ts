import { tryCatch } from "../lib/tryCatch";

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

export class WhepClient {
  private pc: RTCPeerConnection | null = null;
  private resourceUrl: string | null = null;
  private stream: MediaStream | null = null;

  async start(whepUrl: string): Promise<MediaStream> {
    if (this.pc) {
      throw new Error("[whep] already started - call stop() first");
    }

    this.pc = new RTCPeerConnection();
    const remote = new MediaStream();
    this.stream = remote;

    this.pc.addTransceiver("video", { direction: "recvonly" });
    this.pc.addTransceiver("audio", { direction: "recvonly" });
    this.pc.addEventListener("track", (e) => remote.addTrack(e.track));

    const offer = await this.pc.createOffer();
    await this.pc.setLocalDescription(offer);

    const answerSdp = await this.negotiate(whepUrl, offer.sdp!);
    await this.pc.setRemoteDescription({ type: "answer", sdp: answerSdp });
    return remote;
  }

  // MediaMTX 404 until ffmpeg connect
  private async negotiate(whepUrl: string, sdp: string): Promise<string> {
    const deadline = Date.now() + 5000;
    for (;;) {
      const res = await fetch(whepUrl, {
        method: "POST",
        headers: { "Content-Type": "application/sdp" },
        body: sdp,
      });
      if (res.ok) {
        const location = res.headers.get("Location");
        if (location) {
          const { data } = tryCatch(() =>
            new URL(location, whepUrl).toString(),
          );
          this.resourceUrl = data ?? location;
        }
        return res.text();
      }
      if (res.status === 404 && Date.now() < deadline) {
        await sleep(500);
        continue;
      }
      const body = await res.text().catch(() => "");
      throw new Error(`[whep] handshake failed (${res.status}): ${body}`);
    }
  }

  stop(): void {
    if (this.pc) {
      this.pc.close();
      this.pc = null;
    }
    if (this.stream) {
      this.stream.getTracks().forEach((t) => t.stop());
      this.stream = null;
    }
    if (this.resourceUrl) {
      fetch(this.resourceUrl, { method: "DELETE" }).catch(() => {});
      this.resourceUrl = null;
    }
  }
}
