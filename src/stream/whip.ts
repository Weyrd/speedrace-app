/**
 * WhipClient - WebRTC WHIP publisher (v1, webview-side).
 *
 * Flow:
 *   1. Caller acquires a MediaStream (getDisplayMedia / getUserMedia)
 *   2. Call `start(whipUrl, stream)` - negotiates SDP with MediaMTX
 *   3. On success, stream is live on MediaMTX
 *   4. Call `stop()` to tear down the RTCPeerConnection
 *
 * v2 note: when ffmpeg takes over, this file is replaced - nothing else changes.
 * ffpeg can  whip stream? AND save in the same time?
 */

export class WhipClient {
  private pc: RTCPeerConnection | null = null;
  private resourceUrl: string | null = null;

  async start(whipUrl: string, stream: MediaStream): Promise<void> {
    if (this.pc) {
      throw new Error("[whip] already started - call stop() first");
    }

    this.pc = new RTCPeerConnection({
      iceServers: [
        /*
        { urls: "stun:stun.l.google.com:19302" },
        {
          urls: "turn:openrelay.metered.ca:80",
          username: "openrelayproject",
          credential: "openrelayproject",
        },*/
      ],
      // [DEBUG] Décommenter pour forcer le passage par TURN
      // iceTransportPolicy: "relay",
    });

    // --- DEBUT LOGS DEBUG ICE & SDP ---
    /*this.pc.addEventListener("icecandidate", (e) => {
      console.log("[whip] ICE candidate:", e.candidate?.candidate);
    });
    this.pc.addEventListener("icecandidateerror", (e: any) => {
      console.error("[whip] ICE candidate error", e.errorCode, e.errorText, e.url);
    });
    this.pc.addEventListener("iceconnectionstatechange", () => {
      console.log("[whip] ICE state:", this.pc?.iceConnectionState);
    });
    this.pc.addEventListener("connectionstatechange", () => {
      console.log("[whip] PC connectionState:", this.pc?.connectionState);
    });
    // --- FIN LOGS DEBUG ICE & SDP ---

    console.log("[whip] tracks:", stream.getTracks().map(t => `${t.kind} readyState=${t.readyState}`));
    */
    // Add all tracks from the capture stream
    for (const track of stream.getTracks()) {
      this.pc.addTrack(track, stream);
    }

    const offer = await this.pc.createOffer();
    await this.pc.setLocalDescription(offer);
    console.log("[whip] local SDP:", this.pc.localDescription?.sdp);

    // Send the offer immediately without waiting for ICE gathering (Trickle ICE)
    const res = await fetch(whipUrl, {
      method: "POST",
      headers: { "Content-Type": "application/sdp" },
      body: this.pc.localDescription!.sdp,
    });

    if (!res.ok) {
      const body = await res.text().catch(() => "");
      throw new Error(`[whip] handshake failed (${res.status}): ${body}`);
    }

    // Save the resource URL so we can DELETE it on stop (clean WHIP teardown).
    // The Location header may be a relative path - resolve it against the WHIP
    // server origin so the DELETE goes to MediaMTX, not the page origin.
    const location = res.headers.get("Location");
    if (location) {
      try {
        this.resourceUrl = new URL(location, whipUrl).toString();
      } catch {
        this.resourceUrl = location;
      }
    }

    const answerSdp = await res.text();
    await this.pc.setRemoteDescription({ type: "answer", sdp: answerSdp });
    // console.log("[whip] remote SDP:", this.pc.remoteDescription?.sdp);
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
}
