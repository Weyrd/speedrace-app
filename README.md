->  README_STREAM_V2.MD pour ffmpeg

# Tauri + React + Typescript

This template should help get you started developing with Tauri, React and Typescript in Vite.

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

# start the app

npm install
npm run tauri dev

## 13. Notes v2+ (hors scope, à garder en tête)

Ces features sont **hors scope v1** mais l'architecture ci-dessus les anticipe sans coût supplémentaire.

## Possible V2 Improvements

- Add a “Force Update” action in the app linked to the API
- Add a periodic update check (background or interval-based)
- Add a setting to choose the folder path where runs/files are saved
- Add a setting to automatically keep or delete files after upload

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
