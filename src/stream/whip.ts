
import { tryCatch } from "../lib/tryCatch";

export class WhipClient {
  private pc: RTCPeerConnection | null = null;
  private resourceUrl: string | null = null;
  private stream: MediaStream | null = null;

  async start(whipUrl: string, stream: MediaStream): Promise<void> {
    if (this.pc) {
      throw new Error("[whip] already started - call stop() first");
    }

    this.stream = stream;
    this.pc = new RTCPeerConnection({
      iceServers: [
              ],
    });

    for (const track of stream.getTracks()) {
      this.pc.addTrack(track, stream);
    }

    const offer = await this.pc.createOffer();
    await this.pc.setLocalDescription(offer);
    console.log("[whip] local SDP:", this.pc.localDescription?.sdp);

    const res = await fetch(whipUrl, {
      method: "POST",
      headers: { "Content-Type": "application/sdp" },
      body: this.pc.localDescription!.sdp,
    });

    if (!res.ok) {
      const body = await res.text().catch(() => "");
      throw new Error(`[whip] handshake failed (${res.status}): ${body}`);
    }

    const location = res.headers.get("Location");
    if (location) {
      const { data } = tryCatch(() => new URL(location, whipUrl).toString());
      this.resourceUrl = data ?? location;
    }

    const answerSdp = await res.text();
    await this.pc.setRemoteDescription({ type: "answer", sdp: answerSdp });
  }

  stop(): void {
    if (this.stream) {
      this.stream.getTracks().forEach((t) => t.stop());
      this.stream = null;
    }

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
}
