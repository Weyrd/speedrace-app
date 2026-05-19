# Momentum — Tauri Desktop App Spec

> Spec de référence pour l'app Tauri. À lire conjointement avec `R_WS_SPEC.MD` et `R_LOBBY_SPEC.md`.
> Scope v1 : WHIP direct (pas de ffmpeg). Architecture pensée pour accueillir ffmpeg/RTMP, YouTube upload et LiveSplit en v2+.

---

## Table des matières

1. [Vue d'ensemble](#1-vue-densemble)
2. [Tech stack & choix d'architecture](#2-tech-stack--choix-darchitecture)
3. [Authentification](#3-authentification)
4. [WebSocket client](#4-websocket-client)
5. [State machine de l'app](#5-state-machine-de-lapp)
6. [Screens (UI)](#6-screens-ui)
7. [Stream pipeline](#7-stream-pipeline)
8. [System tray](#8-system-tray)
9. [Auto-update](#9-auto-update)
10. [Communication Rust ↔ Webview](#10-communication-rust--webview)
11. [Structure des fichiers](#11-structure-des-fichiers)
12. [Plan d'implémentation v1](#12-plan-dimplémentation-v1)
13. [Notes v2+ (hors scope, à garder en tête)](#13-notes-v2-hors-scope-à-garder-en-tête)

---

## 1. Vue d'ensemble

L'app Tauri est un **processus tray minimaliste**. Elle ne remplace pas le site web — elle le complète. Le joueur fait tout ce qui est lobby/ready/finish dans son navigateur. L'app gère uniquement :

- La connexion persistante au backend (`/ws/app`)
- La capture et la publication du stream (WHIP en v1, ffmpeg RTMP en v2)
- La preview locale avant publication
- L'affichage d'un état minimaliste (en attente, stream actif, erreur)
- La réception des commandes serveur (lobby_setup, countdown, race_results)

**Philosophie UI :** le moins d'écrans possible. Fenêtre petite, toujours accessible depuis le tray. Aucune navigation côté app — c'est le serveur (via WS) qui pilote les transitions d'état.

---

## 2. Tech stack & choix d'architecture

| Couche          | Choix                                        | Raison                                                            |
| --------------- | -------------------------------------------- | ----------------------------------------------------------------- |
| Framework       | **Tauri v2** (Rust + Chromium webview)       | Binaire léger, WebRTC natif dans le webview, accès système Rust   |
| UI              | **React + Vite** (dans le webview)           | Stack déjà connue côté web, réutilisation de composants possibles |
| Styling         | **Tailwind CSS**                             | Cohérence avec le front web                                       |
| Rust async      | **Tokio**                                    | Déjà dans la stack backend, WS reconnect loop                     |
| WS client       | **tokio-tungstenite**                        | Async, compatible Tokio                                           |
| Stockage tokens | **tauri-plugin-store** (chiffré OS keychain) | Secure storage natif par OS                                       |
| WHIP v1         | **WebRTC dans le webview** (browser API)     | Chromium a WebRTC intégré, zéro dépendance Rust                   |
| WHIP/RTMP v2    | **ffmpeg bundlé** (Rust spawn)               | Même interface côté Rust, swap transparent                        |
| Auto-update     | **tauri-plugin-updater**                     | Intégré Tauri, GitHub Releases comme endpoint                     |

### Pourquoi WebRTC dans le webview pour v1 (et pas Rust) ?

Le webview Chromium supporte `getUserMedia` + `RTCPeerConnection` nativement. Faire du WebRTC côté Rust (`webrtc-rs`) est significativement plus complexe pour un résultat identique en v1. La migration vers ffmpeg en v2 ne touchera que la couche stream — l'interface Rust reste la même (`StreamHandle`).

### Auth séparée de la connexion WS web ?

**Non, même JWT.** Le token est émis par le backend au login OAuth, utilisé pour les appels HTTP et pour le handshake WS (`/ws/app?token=<jwt>`). La question de sécurité (token exposé dans l'URL) est acceptable ici : c'est un param de handshake une seule fois, la connexion passe en WSS (TLS), et c'est le standard de facto pour les WS auth (les headers custom ne sont pas supportés partout).

Ce qui est **séparé** en revanche : le flux d'acquisition du token (OAuth via browser system) est géré entièrement côté Rust/Tauri, pas via le webview de l'app. Le webview n'a jamais les tokens en mémoire.

---

## 3. Authentification

### Flux OAuth (PKCE recommandé)

```
1. App génère code_verifier + code_challenge (PKCE)
2. App ouvre le browser système (pas le webview interne) :
   https://momentum.app/auth/desktop?
     client_id=tauri_desktop&
     redirect_uri=momentum://auth/callback&
     code_challenge=<xxx>&
     response_type=code
3. User se connecte sur le site web normalement
4. Backend redirige vers momentum://auth/callback?code=<auth_code>
5. Tauri intercepte le deep link (custom URI scheme "momentum://")
6. App échange le code contre access_token + refresh_token via POST /auth/token
7. Tokens stockés dans le keychain OS via tauri-plugin-store (chiffré)
8. Fenêtre principale : état passe à Connecting
```

**Pourquoi ouvrir le browser système et pas le webview interne ?**

- L'utilisateur voit bien une URL légitime dans son browser habituel (trust visuel)
- Le browser système a déjà les cookies de session si l'utilisateur est connecté sur le web → one-click login possible
- Pas de risque d'interception par le webview de l'app

### Auto-refresh des tokens

Tâche Tokio en background, lancée après le premier login :

```rust
async fn token_refresh_loop(store: Arc<TokenStore>) {
    loop {
        let expires_in = store.time_until_expiry();
        // Refresh 60 secondes avant expiration
        let sleep_duration = expires_in.saturating_sub(Duration::from_secs(60));
        tokio::time::sleep(sleep_duration).await;

        match refresh_access_token(&store.refresh_token()).await {
            Ok(new_tokens) => store.update(new_tokens),
            Err(_) => {
                // Refresh token expiré ou révoqué → déconnexion propre
                store.clear();
                emit_to_webview("auth:logout", ());
            }
        }
    }
}
```

La WS connection utilise le token courant au moment de la (re)connexion. Si la connexion WS drop et qu'un refresh a eu lieu entre-temps, le reconnect loop utilise automatiquement le nouveau token.

### Stockage

```
tauri-plugin-store → momentum_auth.json (chiffré par l'OS)
  ├── access_token: string
  ├── refresh_token: string
  ├── expires_at: ISO8601
  └── user: { id, username, avatar_url }
```

Jamais de token dans le webview. Le Rust layer expose uniquement `get_current_user()` (données non-sensibles) via Tauri command.

---

## 4. WebSocket client

La connexion `/ws/app` est gérée entièrement côté Rust (pas dans le webview). C'est une connexion persistante avec reconnect automatique exponentiel.

### Reconnect loop

```rust
async fn ws_connect_loop(
    token_store: Arc<TokenStore>,
    event_tx: mpsc::Sender<AppEvent>,
    cmd_rx: mpsc::Receiver<WsCommand>,
) {
    let mut backoff = Duration::from_secs(1);
    loop {
        let token = token_store.access_token();
        let url = format!("wss://api.momentum.app/ws/app?token={}", token);

        match connect_ws(&url).await {
            Ok(ws) => {
                backoff = Duration::from_secs(1); // reset sur succès
                handle_ws_session(ws, &event_tx, &cmd_rx).await;
            }
            Err(e) => {
                tracing::warn!("WS connect failed: {e}");
                emit_to_webview("ws:status", WsStatus::Disconnected);
            }
        }

        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(Duration::from_secs(30));
    }
}
```

### Messages entrants (Backend → App)

| Message WS    | Action Rust                                          | Event Webview émis |
| ------------- | ---------------------------------------------------- | ------------------ |
| `lobby_setup` | Stocke `{ lobby_id, stream_key, whip_url }` en state | `ws:lobby_setup`   |
| `countdown`   | Stocke `race_start_at` en state                      | `ws:countdown`     |

_(pas d'autres messages en v1 — `race_results` est web-only)_

### Messages sortants (App → Backend)

| Commande interne                   | Message WS envoyé                                               |
| ---------------------------------- | --------------------------------------------------------------- |
| `WsCommand::StreamReady(lobby_id)` | `{ "event": "stream_ready", "payload": { "lobby_id": "..." } }` |

La couche WS ne connaît pas les concepts métier — elle reçoit des `WsCommand` et envoie du JSON. Le service applicatif décide quand envoyer quoi.

---

## 5. State machine de l'app

```
┌─────────────────┐
│  Unauthenticated │ ← démarrage, ou logout, ou refresh token expiré
└────────┬────────┘
         │ login OAuth réussi
         ▼
┌─────────────────┐
│   Connecting     │ ← WS en cours de connexion
└────────┬────────┘
         │ WS connected
         ▼
┌─────────────────┐
│      Idle        │ ← connecté, en attente de lobby_setup
└────────┬────────┘
         │ lobby_setup reçu
         ▼
┌─────────────────┐
│  StreamSetup     │ ← preview locale, bouton "Publier"
└────────┬────────┘
         │ utilisateur clique "Publier" → WHIP établi → stream_ready envoyé
         ▼
┌─────────────────┐
│  WaitingForStart │ ← stream live, en attente du countdown
└────────┬────────┘
         │ countdown reçu
         ▼
┌─────────────────┐
│   Countdown      │ ← décompte vers race_start_at
└────────┬────────┘
         │ race_start_at atteint
         ▼
┌─────────────────┐
│     Racing       │ ← stream actif, bouton stop/forfeit
└────────┬────────┘
         │ race_results reçu (WS)  OU  utilisateur clique Stop (avec confirmation)
         ▼
         └──────────────────────────────────→ Idle
```

**Transitions d'erreur (toujours vers Idle ou Connecting) :**

- WS drop pendant Idle/StreamSetup/WaitingForStart → Connecting (reconnect loop)
- WS drop pendant Racing → Connecting + backend déclenche forfeit automatiquement
- Stream WHIP drop pendant WaitingForStart/Racing → StreamError → retour StreamSetup

**Note : l'état `Racing` ne dépend pas d'un événement WS entrant pour s'activer.** Le webview compte jusqu'à `race_start_at` de façon autonome. Le WS n'est pas nécessaire pendant la race — sauf pour `race_results` qui déclenche le retour à Idle.

---

## 6. Screens (UI)

L'app a **une seule fenêtre** (petite, 380×520 px, pas redimensionnable en v1). Pas de navigation au sens SPA — c'est l'état Rust qui détermine quel screen afficher. La fenêtre peut être masquée dans le tray.

### Screen 1 — Login

**Quand :** état `Unauthenticated`

```
┌──────────────────────────────────┐
│         🏁 Momentum              │
│                                  │
│    Connecte-toi pour jouer.      │
│                                  │
│   [ Se connecter avec le web ]   │
│                                  │
│   v1.0.0                         │
└──────────────────────────────────┘
```

- Un seul bouton → ouvre le browser système sur la page de login
- Pas de formulaire email/password dans l'app (tout délégué au web)
- Version affichée en bas (utile pour le support, donne aussi le signal visuel que l'updater peut cibler)

### Screen 2 — Idle (En attente de lobby)

**Quand :** état `Idle` (WS connecté, pas de lobby)

```
┌──────────────────────────────────┐
│  ● Connecté   [username]    [⚙️] │
│──────────────────────────────────│
│                                  │
│         En attente               │
│      d'un lobby...               │
│                                  │
│   Rejoins un lobby sur le web    │
│   pour commencer.                │
│                                  │
└──────────────────────────────────┘
```

- Indicateur de connexion WS (vert = connecté, orange = reconnection en cours)
- Username de l'utilisateur connecté
- Icône ⚙️ ouvre le panneau Settings (voir ci-dessous)
- Si WS en cours de reconnect : message "Reconnexion..." + spinner à la place de "Connecté"

### Screen 3 — Stream Setup

**Quand :** état `StreamSetup`

```
┌──────────────────────────────────┐
│  ● Connecté   [username]    [⚙️] │
│──────────────────────────────────│
│  Lobby reçu : [code]             │
│                                  │
│  ┌──────────────────────────┐    │
│  │   PREVIEW CAMÉRA/ÉCRAN   │    │
│  │   (getUserMedia webview) │    │
│  └──────────────────────────┘    │
│                                  │
│  Source : [ Écran ▾ ]            │
│                                  │
│        [ 🔴 Publier ]            │
└──────────────────────────────────┘
```

- La preview démarre **automatiquement** dès réception de `lobby_setup` (pas besoin d'un clic pour voir)
- Sélecteur de source (Écran / Caméra) — en v1, Écran uniquement (garde le sélecteur pour préparer v2)
- Bouton "Publier" → établit la connexion WHIP → envoie `stream_ready` au backend → transition vers `WaitingForStart`
- Si erreur WHIP : message d'erreur inline + bouton "Réessayer"

### Screen 4 — En attente de démarrage (WaitingForStart)

**Quand :** état `WaitingForStart` (stream actif, attente que l'hôte lance la race)

```
┌──────────────────────────────────┐
│  ● Connecté   [username]    [⚙️] │
│──────────────────────────────────│
│  🔴 LIVE — Lobby [code]          │
│                                  │
│         Stream actif ✅          │
│                                  │
│   En attente que l'hôte         │
│   lance la race...               │
│                                  │
│   [ ⏹ Arrêter le stream ]       │
└──────────────────────────────────┘
```

- Indicateur "LIVE" rouge pour rendre l'état du stream évident
- Bouton stop avec **modal de confirmation** (voir ci-dessous)

### Screen 5 — Countdown + Racing

**Quand :** états `Countdown` et `Racing`

```
┌──────────────────────────────────┐
│  ● Connecté   [username]    [⚙️] │
│──────────────────────────────────│
│  🔴 LIVE — Lobby [code]          │
│                                  │
│         ⏱ 00:04:32               │  ← timer depuis race_start_at
│                                  │   (pendant Countdown : décompte inversé)
│                                  │
│   [ ⏹ Arrêter / Forfeit ]       │
└──────────────────────────────────┘
```

- Pendant `Countdown` : affiche le décompte (5, 4, 3, 2, 1...)
- Pendant `Racing` : chrono qui monte depuis `race_start_at`
- Le bouton Arrêter en `Racing` est un **forfeit** → modal de confirmation avec wording explicite : "Arrêter le stream en cours de race déclenche un forfeit. Continuer ?"

### Modal Stop / Forfeit

```
┌──────────────────────────────────┐
│  ⚠️  Arrêter le stream ?         │
│                                  │
│  [En race] : Cela déclenche      │
│  un forfeit automatique.         │
│                                  │
│  [ Annuler ]   [ ⏹ Arrêter ]    │
└──────────────────────────────────┘
```

Le wording change selon l'état (`WaitingForStart` vs `Racing`).

### Panneau Settings (⚙️)

Slide-in ou modal. En v1 : vide sauf pour les placeholders. Structure prévue pour v2+ :

```
┌──────────────────────────────────┐
│  ⚙️  Paramètres             [✕] │
│──────────────────────────────────│
│  Compte                          │
│  → [username]  [ Se déconnecter ]│
│                                  │
│  Démarrage                       │
│  → Lancer au démarrage  [  ○  ] │
│  → Réduire dans le tray [  ●  ] │
│                                  │
│  Stream (bientôt)                │
│  → Qualité    [placeholder]      │
│  → Source     [placeholder]      │
│                                  │
│  YouTube (bientôt)               │
│  → Compte YouTube [placeholder]  │
│                                  │
│  v1.0.0 — [Vérifier les mises   │
│            à jour]               │
└──────────────────────────────────┘
```

---

## 7. Stream pipeline

### v1 — WHIP via webview (WebRTC browser API)

```
getUserMedia({ video: { displaySurface: "monitor" } })
    ↓
RTCPeerConnection
    ↓
WHIP handshake : POST {whip_url}
    body: SDP offer
    Content-Type: application/sdp
    ↓
MediaMTX répond : 201 + SDP answer + Location header
    ↓
Stream live → backend notifié via stream_ready (WS)
```

Le webview gère tout en TypeScript. Le Rust n'a pas besoin de savoir comment fonctionne WHIP — il reçoit juste `"stream_ready"` via un Tauri command quand le TS confirme que c'est live.

```typescript
// src/stream/whip.ts
export class WhipClient {
  private pc: RTCPeerConnection | null = null;

  async start(whipUrl: string, stream: MediaStream): Promise<void> {
    this.pc = new RTCPeerConnection({ iceServers: [] });
    stream.getTracks().forEach((t) => this.pc!.addTrack(t, stream));

    const offer = await this.pc.createOffer();
    await this.pc.setLocalDescription(offer);

    const res = await fetch(whipUrl, {
      method: "POST",
      headers: { "Content-Type": "application/sdp" },
      body: offer.sdp,
    });

    if (!res.ok) throw new Error(`WHIP failed: ${res.status}`);
    const answer = await res.text();
    await this.pc.setRemoteDescription({ type: "answer", sdp: answer });
  }

  stop(): void {
    this.pc?.close();
    this.pc = null;
  }
}
```

### v2 — ffmpeg RTMP (architecture préparée)

En v2, le Rust spawne ffmpeg. L'interface côté Rust est identique à v1 — seule l'implémentation change.

```rust
// Même interface en v1 et v2
pub trait StreamHandle: Send + Sync {
    async fn start(&self, config: StreamConfig) -> Result<()>;
    async fn stop(&self) -> Result<()>;
    fn is_live(&self) -> bool;
}

// v1: envoie une commande au webview
pub struct WhipStreamHandle { ... }

// v2: spawn ffmpeg process
pub struct FfmpegStreamHandle {
    process: Option<Child>,
    // ...
}
```

Le feature flag Tauri `stream-backend` permet de switcher en compile-time si besoin.

### Arrêt du stream (race terminée)

Quand `race_results` est reçu par le WS Rust :

1. Le Rust émet `ws:race_results` vers le webview
2. Le webview appelle `stream.stop()` (ferme le `RTCPeerConnection`)
3. Le webview notify Rust via command `stream_stopped`
4. L'état app transite vers `Idle`

---

## 8. System tray

Comportement par défaut : fermer la fenêtre (`X`) la **masque dans le tray** (ne quitte pas le process).

**Icône tray :**

- Gris — déconnecté / non authentifié
- Vert — connecté, idle
- Rouge clignotant — stream actif

**Menu contextuel tray (clic droit) :**

```
Ouvrir Momentum
────────────────
● Connecté en tant que [username]   ← si connecté (non-cliquable)
────────────────
Quitter
```

**Clic gauche sur l'icône** → show/hide la fenêtre principale.

**Option paramètre "Réduire dans le tray"** (on par défaut) : si l'option est off, `X` quitte vraiment l'app. Ce comportement est configurable dans Settings > Démarrage.

---

## 9. Auto-update

### Architecture (hook posé en v1, fonctionnel en v2)

Tauri intègre `tauri-plugin-updater`. La config dans `tauri.conf.json` :

```json
{
  "plugins": {
    "updater": {
      "active": true,
      "endpoints": [
        "https://github.com/[org]/momentum-desktop/releases/latest/download/latest.json"
      ],
      "dialog": false,
      "pubkey": "..."
    }
  }
}
```

- `dialog: false` → on gère l'UI nous-mêmes (affichage dans Settings)
- Le fichier `latest.json` est généré automatiquement par la CI GitHub Actions au release

### Comportement v1 (minimal)

- Au démarrage : check silencieux
- Si update disponible : un badge apparaît sur l'icône ⚙️ + message dans Settings "Mise à jour disponible — v1.x.x"
- L'utilisateur clique "Mettre à jour" → download + install + restart

### Comportement v2 (prévu)

- Auto-download en background
- Prompt "Redémarrer pour appliquer la mise à jour" une fois téléchargé
- Bloquer si stream actif (ne pas interrompre une race)

### CI — GitHub Actions Release

```yaml
# .github/workflows/release.yml
- name: Build Tauri app
  uses: tauri-apps/tauri-action@v0
  with:
    tagName: v__VERSION__
    releaseName: "Momentum v__VERSION__"
    releaseBody: "..."
    tauriScript: npm run tauri
    args: --target x86_64-pc-windows-msvc
  env:
    AZURE_CLIENT_ID: ${{ secrets.AZURE_CLIENT_ID }}
    AZURE_CLIENT_SECRET: ${{ secrets.AZURE_CLIENT_SECRET }}
    AZURE_TENANT_ID: ${{ secrets.AZURE_TENANT_ID }}
```

L'action génère automatiquement le `latest.json` et le publie sur GitHub Releases.

---

## 10. Communication Rust ↔ Webview

### Rust → Webview (events Tauri)

| Event            | Payload                                                     | Déclenché quand                   |
| ---------------- | ----------------------------------------------------------- | --------------------------------- |
| `auth:state`     | `{ state: "authenticated" \| "unauthenticated", user? }`    | Login, logout, refresh fail       |
| `ws:status`      | `{ status: "connected" \| "connecting" \| "disconnected" }` | Changement état WS                |
| `ws:lobby_setup` | `{ lobby_id, stream_key, whip_url }`                        | Backend envoie lobby_setup        |
| `ws:countdown`   | `{ race_start_at: string }`                                 | Backend envoie countdown          |
| `app:state`      | `AppState` (enum sérialisé)                                 | Toute transition de state machine |

### Webview → Rust (commands Tauri)

| Command                 | Payload        | Action Rust                                           |
| ----------------------- | -------------- | ----------------------------------------------------- |
| `get_app_state`         | —              | Retourne l'état courant                               |
| `get_current_user`      | —              | Retourne `{ username, avatar_url }` depuis TokenStore |
| `notify_stream_ready`   | `{ lobby_id }` | Envoie `stream_ready` via WS                          |
| `notify_stream_stopped` | —              | Nettoie le state stream                               |
| `logout`                | —              | Efface tokens, déconnecte WS, → Unauthenticated       |
| `open_login`            | —              | Ouvre le browser système sur la page de login         |
| `check_for_update`      | —              | Lance le check updater manuellement                   |

**Règle :** le webview ne stocke jamais de tokens. Il demande au Rust ce dont il a besoin. Le Rust est le seul source of truth pour l'état de l'app.

---

## 11. Structure des fichiers

```
momentum-desktop/
├── src-tauri/
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── capabilities/          ← Tauri v2 permissions
│   └── src/
│       ├── main.rs            ← bootstrap Tauri
│       ├── lib.rs             ← app builder, plugin registration
│       ├── state.rs           ← AppState global (Arc<Mutex<...>>)
│       ├── auth/
│       │   ├── mod.rs
│       │   ├── oauth.rs       ← flow PKCE, deep link handler
│       │   ├── token_store.rs ← lecture/écriture tauri-plugin-store
│       │   └── refresh.rs     ← token_refresh_loop
│       ├── ws/
│       │   ├── mod.rs
│       │   ├── client.rs      ← connect loop + reconnect backoff
│       │   ├── handler.rs     ← dispatch messages entrants
│       │   └── commands.rs    ← WsCommand enum
│       ├── stream/
│       │   ├── mod.rs
│       │   ├── handle.rs      ← trait StreamHandle
│       │   └── whip.rs        ← v1: proxy vers webview (v2: ffmpeg ici)
│       ├── tray.rs            ← setup tray + menu + icône state
│       ├── updater.rs         ← check_update, badge notification
│       └── commands.rs        ← Tauri commands exposés au webview
│
├── src/                       ← Webview (React + Vite)
│   ├── main.tsx
│   ├── App.tsx                ← router d'état (switch on AppState)
│   ├── screens/
│   │   ├── Login.tsx
│   │   ├── Idle.tsx
│   │   ├── StreamSetup.tsx
│   │   ├── WaitingForStart.tsx
│   │   └── Racing.tsx
│   ├── components/
│   │   ├── Header.tsx         ← barre commune (status WS, username, ⚙️)
│   │   ├── StopModal.tsx      ← modal confirmation stop/forfeit
│   │   └── Settings.tsx       ← panneau settings slide-in
│   ├── stream/
│   │   └── whip.ts            ← WhipClient (WebRTC)
│   ├── hooks/
│   │   ├── useAppState.ts     ← subscribe aux events Tauri
│   │   └── useCountdown.ts    ← décompte vers race_start_at
│   └── lib/
│       └── tauri.ts           ← wrappers typed autour de invoke/listen
│
├── package.json
└── vite.config.ts
```

---

## 12. Plan d'implémentation v1

Dans l'ordre de dépendances :

| #   | Fichier/Module                      | Action                                                                                       |
| --- | ----------------------------------- | -------------------------------------------------------------------------------------------- |
| 1   | `tauri.conf.json`                   | Config app : nom, identifier, window size, deep link scheme `momentum://`, tray, permissions |
| 2   | `src-tauri/src/state.rs`            | `AppState` enum + `GlobalState` struct                                                       |
| 3   | `src-tauri/src/auth/token_store.rs` | Lecture/écriture tokens via tauri-plugin-store                                               |
| 4   | `src-tauri/src/auth/oauth.rs`       | Ouvrir browser système + intercepter deep link callback                                      |
| 5   | `src-tauri/src/auth/refresh.rs`     | `token_refresh_loop`                                                                         |
| 6   | `src-tauri/src/ws/client.rs`        | WS connect loop + reconnect backoff                                                          |
| 7   | `src-tauri/src/ws/handler.rs`       | Dispatch `lobby_setup` / `countdown` → emit vers webview                                     |
| 8   | `src-tauri/src/commands.rs`         | Tous les Tauri commands exposés                                                              |
| 9   | `src-tauri/src/tray.rs`             | Tray icon + menu + hide-on-close                                                             |
| 10  | `src-tauri/src/updater.rs`          | Check update au démarrage + command manuelle                                                 |
| 11  | `src/stream/whip.ts`                | `WhipClient` WebRTC                                                                          |
| 12  | `src/hooks/useAppState.ts`          | Subscribe events Tauri, state machine côté webview                                           |
| 13  | `src/screens/*`                     | Login → Idle → StreamSetup → WaitingForStart → Racing                                        |
| 14  | `src/components/StopModal.tsx`      | Modal confirmation avec wording contextuel                                                   |
| 15  | `src/components/Settings.tsx`       | Panneau settings (structure + section logout fonctionnelle)                                  |

---

## 13. Notes v2+ (hors scope, à garder en tête)

Ces features sont **hors scope v1** mais l'architecture ci-dessus les anticipe sans coût supplémentaire.

### ffmpeg / RTMP (v2)

- Swap `WhipStreamHandle` → `FfmpegStreamHandle` derrière le trait `StreamHandle`
- Bundler ffmpeg dans le binaire Tauri (`tauri-plugin-shell` avec `sidecar`)
- Config ffmpeg : DXGI + WASAPI, preset `veryfast`, dual tee (local `.mp4` + RTMP)
- L'interface Rust ne change pas, seul `stream/whip.rs` est remplacé par `stream/ffmpeg.rs`

### YouTube Upload (v2)

- Ajouter `upload.rs` dans `src-tauri/src/`
- Backend envoie `upload_ready { upload_ticket, resumable_url }` — app streame le `.mp4` local par chunks
- Progress bar dans l'overlay (nouveau state `Uploading`)
- Bloquer le bouton "Quitter" tant que l'upload n'est pas terminé
- Pas d'OAuth côté app — le `resumable_url` est credential-free (token géré serveur)

### LiveSplit (v2)

- Connexion `ws://localhost:16834/livesplit` depuis Rust
- Forward des split events au backend via la WS existante `/ws/app`
- Icône tray warning si LiveSplit non détecté
- Reconnect loop indépendante de la WS backend

### Qualité de stream (Settings v2)

- Résolution : 720p / 1080p / source
- Framerate : 30 / 60 fps
- Bitrate : auto / manuel
- Source audio : loopback système / micro / les deux
- Ces settings sont passés dans `StreamConfig` → `StreamHandle::start()`

### Multi-plateforme (v3+)

- v1 : Windows uniquement (DXGI + WASAPI dépendent de Windows)
- v2 : macOS possible une fois ffmpeg en place (AVFoundation + CoreAudio à la place de DXGI + WASAPI)
- Linux : OBS virtual camera workaround si demandé par la communauté

### Code signing (à faire avant distribution publique) (V4+)

- Azure Artifact Signing (~$10/mois) intégré dans le GitHub Actions release workflow
- `TAURI_SIGNING_PRIVATE_KEY` : clé Ed25519 pour la signature des mises à jour Tauri (différente du signing Windows)
- SmartScreen: la réputation se construit sur les premières semaines de distribution
