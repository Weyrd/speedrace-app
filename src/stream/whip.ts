
export class WhipClient {
  private pc: RTCPeerConnection | null = null;
  private resourceUrl: string | null = null; 


  async start(whipUrl: string, stream: MediaStream): Promise<void> {
    if (this.pc) {
      throw new Error("[whip] already started — call stop() first");
    }

    this.pc = new RTCPeerConnection({
      iceServers: [],
    });

    for (const track of stream.getTracks()) {
      this.pc.addTrack(track, stream);
    }

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

    this.resourceUrl = res.headers.get("Location") ?? null;

    const answerSdp = await res.text();
    await this.pc.setRemoteDescription({ type: "answer", sdp: answerSdp });
  }


  stop(): void {
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

  private _gatherCompleteOffer(): Promise<RTCSessionDescriptionInit> {
    return new Promise(async (resolve, reject) => {
      if (!this.pc) return reject(new Error("[whip] no peer connection"));

      const offer = await this.pc.createOffer();
      await this.pc.setLocalDescription(offer);

      if (this.pc.iceGatheringState === "complete") {
        resolve(this.pc.localDescription!);
        return;
      }

      const timeout = setTimeout(() => {
        reject(new Error("[whip] ICE gathering timed out after 10s"));
      }, 10_000);

      this.pc.onicegatheringstatechange = () => {
        if (this.pc?.iceGatheringState === "complete") {
          clearTimeout(timeout);
          resolve(this.pc.localDescription!);
        }
      };
    });
  }
}
