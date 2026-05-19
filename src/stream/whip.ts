//TODO: revoir ce fihier/delete les commentaires etc
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
 */

export class WhipClient {
  private pc: RTCPeerConnection | null = null;
  private resourceUrl: string | null = null; // WHIP resource URL from Location header

  /**
   * Starts publishing `stream` to `whipUrl`.
   * Throws if the WHIP handshake fails for any reason.
   */
  async start(whipUrl: string, stream: MediaStream): Promise<void> {
    if (this.pc) {
      throw new Error("[whip] already started — call stop() first");
    }

    this.pc = new RTCPeerConnection({
      // No ICE servers needed for a direct MediaMTX setup.
      // Add STUN/TURN here if the server is behind NAT in the future.
      iceServers: [],
    });

    // Add all tracks from the capture stream
    for (const track of stream.getTracks()) {
      this.pc.addTrack(track, stream);
    }

    // Wait for ICE gathering to finish before sending the offer.
    // This avoids trickle-ICE which MediaMTX doesn't support in WHIP mode.
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

  /**
   * Stops the stream and cleans up the WHIP resource on the server.
   */
  stop(): void {
    // Tear down WebRTC — this stops the stream on MediaMTX's end too,
    // but we also DELETE the resource to be explicit.
    if (this.pc) {
      this.pc.close();
      this.pc = null;
    }

    // Best-effort DELETE — don't await, don't throw
    if (this.resourceUrl) {
      fetch(this.resourceUrl, { method: "DELETE" }).catch(() => {});
      this.resourceUrl = null;
    }
  }

  /**
   * Returns true if the RTCPeerConnection is in a connected state.
   */
  isLive(): boolean {
    return (
      this.pc !== null &&
      (this.pc.connectionState === "connected" ||
        this.pc.connectionState === "connecting")
    );
  }

  // ── Private helpers ──────────────────────────────────────────────────────

  /**
   * Creates an offer and waits for ICE gathering to complete before returning.
   * MediaMTX requires all candidates to be in the SDP — no trickle-ICE.
   */
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
