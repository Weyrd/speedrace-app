# Tauri + React + Typescript

This template should help get you started developing with Tauri, React and Typescript in Vite.

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

# start the app

npm install
npm run tauri dev

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
