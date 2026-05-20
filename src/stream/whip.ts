/**
 * WhipClient — WebRTC WHIP publisher (v1, webview-side).
 *
 * Flow:
 *   1. Caller acquires a MediaStream (getDisplayMedia / getUserMedia)
 *   2. Call `start(whipUrl, stream)` — negotiates SDP with MediaMTX
 *   3. On success, stream is live on MediaMTX
 *   4. Call `stop()` to tear down the RTCPeerConnection
 *
 * v2 note: when ffmpeg takes over, this file is replaced — nothing else changes.
 * ffpeg can  whip stream? AND save in the same time?
 */

export class WhipClient {
  private pc: RTCPeerConnection | null = null;
  private resourceUrl: string | null = null;

  async start(whipUrl: string, stream: MediaStream): Promise<void> {
    if (this.pc) {
      throw new Error("[whip] already started — call stop() first");
    }

    this.pc = new RTCPeerConnection({
      iceServers: [{ urls: "stun:stun.l.google.com:19302" }],
    });

    // Add all tracks from the capture stream
    for (const track of stream.getTracks()) {
      this.pc.addTrack(track, stream);
    }

    // Wait for ICE gathering to finish before sending the offer.
    const offer = await this._gatherCompleteOffer();

    const res = await fetch(whipUrl, {
      method: "POST",
      headers: { "Content-Type": "application/sdp" },
      body: offer.sdp,
    });

    if (!res.ok) {
      const body = await res.text().catch(() => "");
      throw new Error(`[whip] handshake failed (${res.status}): ${body}`);
    }

    // Save the resource URL so we can DELETE it on stop (clean WHIP teardown)
    this.resourceUrl = res.headers.get("Location") ?? null;

    const answerSdp = await res.text();
    await this.pc.setRemoteDescription({ type: "answer", sdp: answerSdp });
  }

  stop(): void {
    // Tear down WebRTC/Delete whipe ressource
    if (this.pc) {
      this.pc.close();
      this.pc = null;
    }

    if (this.resourceUrl) {
      fetch(this.resourceUrl, { method: "DELETE" }).catch(() => {});
      this.resourceUrl = null;
    }
  }

  isLive(): boolean {
    return (
      this.pc !== null &&
      (this.pc.connectionState === "connected" ||
        this.pc.connectionState === "connecting")
    );
  }

  // Heleper
  // create WHIP offer and gather ICE candidates (soft timeout like the PoC)
  private _gatherCompleteOffer(): Promise<RTCSessionDescriptionInit> {
    return new Promise(async (resolve, reject) => {
      if (!this.pc) return reject(new Error("[whip] no peer connection"));

      const offer = await this.pc.createOffer();
      await this.pc.setLocalDescription(offer);

      // If already gathered (e.g. no network interfaces), resolve immediately
      if (this.pc.iceGatheringState === "complete") {
        resolve(this.pc.localDescription!);
        return;
      }

      // Resolve after 3s regardless — host candidates are available immediately,
      // STUN candidates may or may not arrive in time but aren't required for
      // direct server connections.
      const timeout = setTimeout(() => {
        resolve(this.pc!.localDescription!);
      }, 3_000);

      this.pc.onicegatheringstatechange = () => {
        if (this.pc?.iceGatheringState === "complete") {
          clearTimeout(timeout);
          resolve(this.pc.localDescription!);
        }
      };
    });
  }
}
