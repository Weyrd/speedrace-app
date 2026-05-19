# Momentum App — Peer Review

> Scope : **Tauri desktop client** (`momentum-app`)  
> Review type : WebSocket correctness + architecture / organisation  
> Spec référence : `R_LOBBY_SPEC.md` (v1, traitée comme "proche mais pas 100 % à jour")

---

## Table des matières

1. [Résumé exécutif](#1-résumé-exécutif)
2. [Tests WebSocket — résultats](#2-tests-websocket--résultats)
3. [Bugs & problèmes — détail](#3-bugs--problèmes--détail)
4. [Architecture — analyse](#4-architecture--analyse)
5. [Liste des mouvements de fichiers / fonctions](#5-liste-des-mouvements-de-fichiersfoncions)
6. [Plan de patch prioritaire](#6-plan-de-patch-prioritaire)

---

## 1. Résumé exécutif

| Catégorie    | Bugs critiques | Problèmes majeurs | Code mort / à nettoyer |
| ------------ | -------------- | ----------------- | ---------------------- |
| WebSocket    | 3              | 2                 | 3                      |
| Architecture | 0              | 2                 | 4                      |
| **Total**    | **3**          | **4**             | **7**                  |

**Les 3 bugs critiques à patcher immédiatement :**

- `get_lobby_state` n'est **pas enregistré** dans `invoke_handler` → crash au démarrage/session restore.
- Après un **login frais**, la transition `Connecting → Idle` ne se fait jamais côté frontend.
- `ws_connect_loop` peut être **spawné deux fois** en simultané si session restore + login callback se chevauchent.

---

## 2. Tests WebSocket — résultats

| #    | Test                                                     | Résultat                           | Fichier(s) concerné(s)                          |
| ---- | -------------------------------------------------------- | ---------------------------------- | ----------------------------------------------- |
| T-01 | `get_lobby_state` enregistré dans invoke_handler         | ❌ FAIL                            | `lib.rs`                                        |
| T-02 | Transition `Connecting → Idle` après login frais         | ❌ FAIL                            | `ws/client.rs`, `useAppState.ts`                |
| T-03 | Double spawn de `ws_connect_loop` / `token_refresh_loop` | ❌ FAIL                            | `lib.rs`, `auth/oauth.rs`                       |
| T-04 | `WsCommand::StreamStopped` — dead code jamais émis       | ⚠️ WARN                            | `commands.rs`, `ws/commands.rs`, `ws/client.rs` |
| T-05 | Nettoyage de `ws_cmd_tx` après `Disconnect`              | ⚠️ WARN                            | `ws/client.rs`                                  |
| T-06 | `safeListen` — gestion race unmount avant resolve        | ✅ PASS                            | `lib/tauri.ts`                                  |
| T-07 | Reconnect WS — backoff exponentiel + refresh token       | ✅ PASS                            | `ws/client.rs`                                  |
| T-08 | Deadlock potentiel sur `handle_message` dans select!     | ✅ PASS (fragile)                  | `ws/client.rs`, `ws/handler.rs`                 |
| T-09 | `onRaceResults` — reset vers Idle côté frontend          | ✅ PASS (déviation intentionnelle) | `useAppState.ts`                                |
| T-10 | `formatElapsed` — Hook React dans une fonction régulière | ❌ FAIL (dead code)                | `Racing.tsx`                                    |
| T-11 | `APP_STATE` event — défini mais jamais écouté            | ❌ FAIL (dead code)                | `events.ts`, `commands.rs`                      |
| T-12 | `getAppState()` exporté mais jamais appelé               | ⚠️ WARN (dead code)                | `lib/tauri.ts`                                  |
| T-13 | Alignement spec — `lobby_setup` queuing pour app offline | ℹ️ INFO                            | `ws/handler.rs`                                 |

---

## 3. Bugs & problèmes — détail

---

### ❌ T-01 — `get_lobby_state` non enregistré (CRITIQUE)

**Fichier :** `src-tauri/src/lib.rs`

**Problème :**  
`commands::get_lobby_state` est défini dans `commands.rs` et appelé depuis le frontend via `invoke("get_lobby_state")`. Mais il est absent de la liste `tauri::generate_handler![...]` dans `lib.rs`.

```rust
// lib.rs — invoke_handler actuel
.invoke_handler(tauri::generate_handler![
    commands::get_app_state,
    commands::open_login,
    commands::get_current_user,
    commands::logout,
    commands::notify_stream_ready,
    commands::notify_stream_stopped,
    // ❌ commands::get_lobby_state  ← MANQUANT
])
```

`useAppState.ts` appelle `getLobbyState()` au montage dans un `useEffect`. Au démarrage, cet invoke retourne une erreur « command not found » → le frontend reste sur l'état initial (Unauthenticated) même si une session était active.

**Fix :**

```rust
.invoke_handler(tauri::generate_handler![
    commands::get_app_state,
    commands::open_login,
    commands::get_current_user,
    commands::logout,
    commands::notify_stream_ready,
    commands::notify_stream_stopped,
    commands::get_lobby_state, // ← ajouter
])
```

---

### ❌ T-02 — `Connecting → Idle` jamais transitionné après login frais (CRITIQUE)

**Fichiers :** `ws/client.rs` (Rust), `hooks/useAppState.ts` (frontend)

**Problème :**  
Flux après un login frais :

1. `handle_callback` émet `auth:state { state: "authenticated" }`
2. Frontend (`onAuthState`) → `patch({ appState: AppState.Connecting })`
3. WS se connecte → `emit_ws_status` met à jour l'état Rust en `Idle` et émet `ws:status: connected`
4. Frontend (`onWsStatus`) → `patch({ wsStatus: "connected" })` — **seulement wsStatus, pas appState**
5. ❌ Le frontend reste bloqué sur l'écran "Connexion..." indéfiniment

Le mécanisme Rust (step 3) met bien à jour son état interne, mais n'émet pas d'événement `app:state` pour notifier le frontend.

La session restore fonctionne correctement car `getLobbyState()` est appelé au montage — mais ce n'est pas le cas pour un login après démarrage.

**Fix (option A — recommandée) :** Faire émettre `app:state` dans `emit_ws_status` quand la transition `Connecting → Idle` se produit :

```rust
// ws/client.rs — emit_ws_status
pub fn emit_ws_status(app: &AppHandle, state: &SharedState, status: WsStatus) {
    let transitioned_to_idle;
    {
        let mut guard = state.lock().unwrap();
        guard.ws_status = status.clone();
        transitioned_to_idle = status == WsStatus::Connected
            && guard.app_state == AppState::Connecting;
        if transitioned_to_idle {
            guard.app_state = AppState::Idle;
        }
    }
    let _ = app.emit(WS_STATUS, status);
    if transitioned_to_idle {
        let _ = app.emit(crate::events::APP_STATE, AppState::Idle);
    }
}
```

**Fix (option B) :** Le frontend écoute `APP_STATE` et met à jour `appState` en conséquence (requiert aussi d'écouter l'événement dans `useAppState.ts`).

---

### ❌ T-03 — Double spawn de `ws_connect_loop` et `token_refresh_loop` (CRITIQUE)

**Fichiers :** `src-tauri/src/lib.rs`, `src-tauri/src/auth/oauth.rs`

**Problème :**  
Les deux fonctions sont spawnées à deux endroits indépendants :

| Fonction             | `lib.rs::restore_session` | `oauth.rs::handle_callback` |
| -------------------- | ------------------------- | --------------------------- |
| `ws_connect_loop`    | ✅ spawnée                | ✅ spawnée                  |
| `token_refresh_loop` | ✅ spawnée                | ✅ spawnée                  |

Un démarrage avec session valide lance `restore_session`. La session peut expirer, donc l'utilisateur se reconnecte → `handle_callback` spawne à nouveau les deux loops. Résultat : deux loops WS concurrentes se battent pour `ws_cmd_tx`. La seconde écrase la référence dans l'état, rendant la première orpheline et indestructible jusqu'à la fermeture de l'app.

Même problème si `restore_session` échoue lentement (réseau) et que l'utilisateur clique "Se connecter" avant la fin.

**Fix :** Introduire un `AtomicBool` ou vérifier dans `restore_session` / `handle_callback` si une loop est déjà active avant de spawner.

```rust
// state.rs
pub struct GlobalState {
    // ...
    pub ws_loop_running: bool, // ← ajouter
}

// Avant de spawner ws_connect_loop :
{
    let mut guard = state.lock().unwrap();
    if guard.ws_loop_running { return; } // déjà actif
    guard.ws_loop_running = true;
}
```

Remettre à `false` quand la loop se termine (fin de `ws_connect_loop`).

---

### ⚠️ T-04 — `WsCommand::StreamStopped` : dead code, jamais envoyé

**Fichiers :** `ws/commands.rs`, `commands.rs`, `ws/client.rs`

**Problème :**  
`WsCommand::StreamStopped { lobby_id }` est défini et géré dans `ws/client.rs` (enverrait `{"type":"stream_stopped","lobby_id":"..."}` au backend). Mais `notify_stream_stopped` dans `commands.rs` n'enqueues jamais cette commande — il se contente de réinitialiser l'état local et d'émettre `APP_STATE`.

Deux cas possibles :

- **Intentionnel** : la spec dit que le forfeit se déclenche sur disconnect WS Tauri, pas sur un message explicite. Dans ce cas, supprimer le variant `StreamStopped`.
- **Bug** : le backend devrait recevoir un message `stream_stopped`. Dans ce cas, il faut l'envoyer depuis `notify_stream_stopped`.

En l'état, le variant est du dead code trompeur.

**Fix :** Soit supprimer `WsCommand::StreamStopped`, soit l'envoyer depuis `notify_stream_stopped` :

```rust
// commands.rs — notify_stream_stopped
pub fn notify_stream_stopped(state: State<SharedState>, app: AppHandle) -> Result<(), String> {
    let (sender, lobby_id) = {
        let mut guard = state.lock().map_err(|e| e.to_string())?;
        let lobby_id = guard.lobby.as_ref().map(|l| l.lobby_id.clone());
        guard.app_state = AppState::Idle;
        guard.lobby = None;
        guard.race_start_at = None;
        (guard.ws_cmd_tx.clone(), lobby_id)
    };
    if let (Some(tx), Some(id)) = (sender, lobby_id) {
        let _ = tx.try_send(WsCommand::StreamStopped { lobby_id: id });
    }
    let _ = app.emit(APP_STATE, AppState::Idle);
    Ok(())
}
```

---

### ⚠️ T-05 — `ws_cmd_tx` non nettoyé après `Disconnect`

**Fichier :** `ws/client.rs`

**Problème :**  
Quand `WsCommand::Disconnect | None` est reçu, la fonction `ws_connect_loop` retourne (`return`). Le receiver (`rx`) est droppé. Mais `guard.ws_cmd_tx` contient toujours le sender (`tx`). Un appel ultérieur à `try_send` (ex : `notify_stream_ready`) mettra le message en queue sans qu'aucun receiver ne l'écoute jamais.

```rust
Some(WsCommand::Disconnect) | None => {
    let _ = write.send(Message::Close(None)).await;
    emit_ws_status(&app, &state, WsStatus::Disconnected);
    // ❌ guard.ws_cmd_tx non nettoyé ici
    return;
}
```

**Fix :**

```rust
Some(WsCommand::Disconnect) | None => {
    let _ = write.send(Message::Close(None)).await;
    emit_ws_status(&app, &state, WsStatus::Disconnected);
    {
        let mut guard = state.lock().unwrap();
        guard.ws_cmd_tx = None; // ← nettoyer
    }
    return;
}
```

---

### ❌ T-10 — `formatElapsed` : Hook React dans une fonction régulière (dead code)

**Fichier :** `src/components/Racing.tsx`

**Problème :**  
La fonction `formatElapsed` appelle `useSyncExternalStore` — un Hook React — directement à l'intérieur d'une fonction non-composant. C'est une violation des règles des Hooks. La fonction est « neutralisée » avec `void formatElapsed` et commentée comme incorrecte, mais elle est toujours présente dans le fichier.

Avec `noUnusedLocals: true` dans `tsconfig.json`, TypeScript peut la signaler (selon la version). Elle génère de la confusion pour quiconque lit le fichier.

**Fix :** Supprimer entièrement `formatElapsed` et son `void formatElapsed` en bas du fichier. La logique correcte est déjà implémentée directement dans le composant `Racing`.

---

### ❌ T-11 — `APP_STATE` event : défini mais jamais consommé côté frontend

**Fichiers :** `src/lib/events.ts`, `src-tauri/src/events.rs`, `commands.rs`

**Problème :**  
`APP_STATE = "app:state"` est défini dans les deux `events` files. Le Rust l'émet dans `notify_stream_stopped`. Mais `useAppState.ts` ne l'écoute jamais. Résultat : l'événement n'a aucun effet côté frontend.

Ce n'est un vrai bug que si le fix de T-02 passe par l'option A (émettre `APP_STATE` depuis `emit_ws_status`). Dans ce cas, il faut aussi écouter l'événement dans `useAppState.ts`.

**Fix (si option A pour T-02) :** Ajouter dans le second `useEffect` de `useAppState.ts` :

```typescript
import { APP_STATE } from "../lib/events";

// dans le useEffect d'écoute des events :
listen<AppState>(APP_STATE, (e) => {
    patch({ appState: e.payload });
}),
```

---

### ℹ️ T-13 — Spec : queuing de `lobby_setup` pour app offline

**Note :** La spec dit que si la Tauri app n'est pas ouverte quand un joueur rejoint, le message `lobby_setup` est mis en queue et envoyé à la prochaine connexion WS. Cette logique est côté **serveur** — le client actuel n'a pas besoin de la gérer. Le `GET /lobby/current` dans `restore_session` (`lobby::fetch_current_lobby`) couvre le cas du reconnect. ✅ Aucune action nécessaire.

---

## 4. Architecture — analyse

### Vue d'ensemble

```
src/
  components/      ← UI screens + quelques mini-composants partagés mal placés
  hooks/           ← useAppState (bien isolé)
  lib/             ← tauri bridge + event constants (bien)
  stream/          ← WhipClient (bien isolé)
  types.ts         ← types globaux (bien)

src-tauri/src/
  auth/            ← oauth, refresh, token_store (bien séparé)
  ws/              ← client, handler, commands (bien séparé)
  stream/          ← handler trait + whip stub (à nettoyer)
  commands.rs      ← Tauri commands (trop de responsabilités mixées)
  lib.rs           ← entry point + restore_session (trop gros)
  state.rs         ← GlobalState (bien)
  lobby.rs         ← fetch lobby HTTP (bien isolé)
  config.rs        ← constantes (bien)
  events.rs        ← event names (bien)
```

### Problèmes d'organisation identifiés

---

#### A-01 — `LivePill` et `LobbyBadge` dans `WaitingForStart.tsx` (mauvaise place)

`Racing.tsx` importe ces composants depuis `WaitingForStart.tsx` :

```typescript
import { LivePill, LobbyBadge } from "./WaitingForStart";
```

Des composants UI partagés ne doivent pas vivre dans un fichier de screen. Si `WaitingForStart` est supprimé ou refactorisé, `Racing.tsx` est cassé.

**→ Déplacer vers `src/components/ui/RaceStatus.tsx`** (voir section 5).

---

#### A-02 — `handle_callback` spawne des loops d'infrastructure dans le module auth

`auth/oauth.rs::handle_callback` spawne `ws_connect_loop` et `token_refresh_loop`. La gestion du cycle de vie de l'application (démarrer les loops de fond) ne devrait pas être responsabilité du module d'authentification.

**→ Extraire dans un module `lifecycle.rs`** (voir section 5).

---

#### A-03 — `restore_session` dans `lib.rs` — fonction trop dense

`restore_session` dans `lib.rs` fait : refresh de token, fetch du lobby courant, mise à jour de l'état global, émission d'événement auth, spawn des loops de fond. C'est trop pour une seule fonction dans l'entry point.

**→ Déplacer vers `lifecycle.rs`** et décomposer.

---

#### A-04 — `stream/whip.rs` entièrement commenté

Tout le contenu est dans un bloc `/* ... */`. Le module est déclaré dans `stream/mod.rs` mais ne contient rien d'actif.

**→ Supprimer** `stream/whip.rs` et son entrée dans `stream/mod.rs`. La v2 (ffmpeg) créera son propre fichier le moment venu.

---

#### A-05 — `notify_stream_stopped` émet `APP_STATE` — side effect inattendu dans un command

Un Tauri command qui émet directement un event applicatif bypass l'architecture normale (où c'est le WS handler ou un lifecycle module qui émet ces events). Difficile à tracer.

**→ L'émission d'events d'état devrait passer par une fonction centralisée** (ex. dans `lifecycle.rs` ou `state.rs`).

---

#### A-06 — `getAppState()` exporté mais jamais utilisé dans `tauri.ts`

```typescript
export async function getAppState(): Promise<AppState> {
  return invoke<AppState>("get_app_state");
}
```

Non appelé nulle part (seul `getLobbyState()` est utilisé). Dead code.

**→ Supprimer** ou garder avec un commentaire `// for future use`.

---

## 5. Liste des mouvements de fichiers/fonctions

Uniquement les mouvements nécessaires — pas de réorganisation cosmétique.

### Frontend (`src/`)

| Élément                          | Fichier actuel                   | Fichier cible                            | Raison                      |
| -------------------------------- | -------------------------------- | ---------------------------------------- | --------------------------- |
| `LivePill` (composant)           | `components/WaitingForStart.tsx` | `components/ui/RaceStatus.tsx` (nouveau) | Partagé entre screens       |
| `LobbyBadge` (composant)         | `components/WaitingForStart.tsx` | `components/ui/RaceStatus.tsx` (nouveau) | Partagé entre screens       |
| `formatElapsed` (fonction morte) | `components/Racing.tsx`          | **Supprimer**                            | Dead code + violation Hooks |
| `getAppState()`                  | `lib/tauri.ts`                   | **Supprimer ou annoter**                 | Non utilisé                 |
| Listener `APP_STATE`             | absent                           | `hooks/useAppState.ts`                   | Nécessaire pour fix T-02    |

### Backend Rust (`src-tauri/src/`)

| Élément                       | Fichier actuel                   | Fichier cible                | Raison                             |
| ----------------------------- | -------------------------------- | ---------------------------- | ---------------------------------- |
| `restore_session()`           | `lib.rs`                         | `lifecycle.rs` (nouveau)     | Trop de logique dans l'entry point |
| Spawn de `ws_connect_loop`    | `auth/oauth.rs::handle_callback` | `lifecycle.rs`               | Infrastructure ≠ auth              |
| Spawn de `token_refresh_loop` | `auth/oauth.rs::handle_callback` | `lifecycle.rs`               | Infrastructure ≠ auth              |
| `stream/whip.rs`              | `src-tauri/src/stream/whip.rs`   | **Supprimer**                | Entièrement commenté               |
| Entrée `pub mod whip`         | `stream/mod.rs`                  | **Supprimer la ligne**       | Idem                               |
| `WsCommand::StreamStopped`    | `ws/commands.rs`                 | **Supprimer ou implémenter** | Dead code                          |

---

## 6. Plan de patch prioritaire

### 🔴 P0 — À patcher avant la prochaine session de test

1. **T-01** — Ajouter `commands::get_lobby_state` dans `invoke_handler` (1 ligne, `lib.rs`)
2. **T-03** — Protéger contre le double spawn avec un flag `ws_loop_running` dans `GlobalState`
3. **T-02** — Émettre `APP_STATE: Idle` depuis `emit_ws_status` quand `Connecting → Idle`; écouter `APP_STATE` dans `useAppState.ts`

### 🟡 P1 — Nettoyage important (prochaine PR)

4. **T-05** — Nettoyer `ws_cmd_tx` dans l'état après `Disconnect`
5. **T-04** — Décider et implémenter : envoyer `stream_stopped` au backend, ou supprimer le variant `StreamStopped`
6. **T-10** — Supprimer `formatElapsed` et le `void formatElapsed` de `Racing.tsx`
7. **T-11** — Soit écouter `APP_STATE` sur le frontend, soit le supprimer des deux `events` files

### 🟢 P2 — Refacto architecture (backlog)

8. **A-01** — Extraire `LivePill` / `LobbyBadge` vers `components/ui/RaceStatus.tsx`
9. **A-02 + A-03** — Créer `lifecycle.rs`, y déplacer `restore_session` et les spawns de loops
10. **A-04** — Supprimer `stream/whip.rs` et sa déclaration de module
11. **A-06** — Supprimer ou annoter `getAppState()` dans `tauri.ts`

---

_Fin du rapport — généré le 2026-05-20_
