# 🚀 NomadFlow

**Terminal mobile résilient avec assistant IA pour le développement nomade**

NomadFlow est une application mobile open source (React Native) qui permet d'accéder à un terminal distant résilient, optimisé pour le développement mobile avec assistance IA.

![License](https://img.shields.io/badge/license-MIT-blue.svg)
![Platform](https://img.shields.io/badge/platform-iOS%20%7C%20Android-lightgrey.svg)
![React Native](https://img.shields.io/badge/React%20Native-0.73-61dafb.svg)

## ✨ Fonctionnalités

### 🎯 Workflow Ultra Fluide
- **Sélection en 3 étapes** : Serveur → Repo → Feature → Terminal prêt !
- **Zéro commande manuelle** : l'environnement est automatiquement configuré
- **Agent IA pré-lancé** : Claude, Ollama ou votre agent personnalisé vous attend

### 📱 Application Mobile
- **Compatible iOS et Android** via React Native
- **Terminal xterm.js** intégré avec rendu natif
- **Persistance des sessions** grâce à tmux
- **Mode hors-ligne** avec cache local des sélections récentes

### 🔒 Connexion Sécurisée
- **WebSocket sécurisé (WSS)** vers votre serveur
- **Authentification par secret partagé** : protège l'API et le terminal
- **Auto-reconnexion** intelligente avec backoff

### 🌿 Gestion des Environnements
- **Git worktrees** : une branche = un environnement isolé
- **Sessions tmux** persistantes par feature
- **Scripts serveur** pour automatiser la création/cleanup

## 📸 Screenshots

```
┌─────────────────────────────────────────┐
│  🖥️ Serveurs          ⚙️              │
├─────────────────────────────────────────┤
│                                         │
│  ┌─────────────────────────────────┐   │
│  │ 🖥️ Mon Serveur Dev              │   │
│  │    wss://192.168.1.100:7681     │   │
│  │    Connecté il y a 5 min        │   │
│  └─────────────────────────────────┘   │
│                                         │
│  ┌─────────────────────────────────┐   │
│  │ 🖥️ Serveur Cloud               │   │
│  │    wss://dev.example.com        │   │
│  │    Jamais connecté              │   │
│  └─────────────────────────────────┘   │
│                                         │
│                              [+]        │
└─────────────────────────────────────────┘
```

## 🚀 Quick Start

### Installation de la CLI

**macOS / Linux :**
```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/fab-uleuh/NomadFlowCode/releases/latest/download/nomadflow-installer.sh | sh
```

**Windows (PowerShell) :**
```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/fab-uleuh/NomadFlowCode/releases/latest/download/nomadflow-installer.ps1 | iex"
```

**Depuis les sources (nécessite Rust) :**
```bash
git clone https://github.com/fab-uleuh/NomadFlowCode.git
cd NomadFlowCode/nomadflow-rs
cargo install --path .
```

### Utilisation

```bash
# Lancer le TUI wizard (serveur + interface interactive)
nomadflow

# Lancer le serveur HTTP seul (mode headless/Docker)
nomadflow serve

# Afficher le statut tmux
nomadflow --status

# S'attacher directement à une session
nomadflow --attach <feature>
```

### Configuration

```bash
# Le fichier de configuration est créé automatiquement au premier lancement
nano ~/.nomadflowcode/config.toml
```

### Côté Mobile

1. **Cloner le repo** :
```bash
git clone https://github.com/fab-uleuh/NomadFlowCode.git
cd NomadFlowCode
```

2. **Installer les dépendances** :
```bash
npm install
# ou
yarn install
```

3. **iOS** :
```bash
cd ios && pod install && cd ..
npm run ios
```

4. **Android** :
```bash
npm run android
```

## 📋 Prérequis

### Serveur
- Linux/macOS avec accès SSH
- **ttyd** (terminal web)
- **tmux** (multiplexeur de terminal)
- **Git** avec support worktrees
- Optionnel : **Ollama**, **Claude CLI**, ou autre agent IA

### Mobile
- Node.js 18+
- React Native CLI
- Xcode (iOS) ou Android Studio (Android)

## 🏗️ Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Mobile App                           │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌────────┐ │
│  │ Servers  │→ │  Repos   │→ │ Features │→ │Terminal│ │
│  └──────────┘  └──────────┘  └──────────┘  └────────┘ │
│       │                                         │      │
│       └──────────── WebSocket ──────────────────┘      │
└────────────────────────┬────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────┐
│                    Server                               │
│  ┌──────────────────────────────────────────────────┐  │
│  │                    ttyd                           │  │
│  │         (WebSocket → PTY bridge)                  │  │
│  └──────────────────────┬───────────────────────────┘  │
│                         │                               │
│  ┌──────────────────────▼───────────────────────────┐  │
│  │                    tmux                           │  │
│  │  ┌─────────┐  ┌─────────┐  ┌─────────┐          │  │
│  │  │ Window1 │  │ Window2 │  │ Window3 │  ...     │  │
│  │  │feature-a│  │feature-b│  │  main   │          │  │
│  │  └────┬────┘  └────┬────┘  └────┬────┘          │  │
│  └───────┼────────────┼────────────┼────────────────┘  │
│          │            │            │                    │
│  ┌───────▼────┐ ┌─────▼──────┐ ┌──▼───┐               │
│  │  Worktree  │ │  Worktree  │ │ Main │               │
│  │ feature-a  │ │  feature-b │ │ Repo │               │
│  └────────────┘ └────────────┘ └──────┘               │
│                                                        │
│  ┌──────────────────────────────────────────────────┐  │
│  │              AI Agent (Claude/Ollama)            │  │
│  └──────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
```

## 📁 Structure du Projet

```
NomadFlowCode/
├── src/
│   ├── App.tsx                 # Point d'entrée
│   ├── screens/
│   │   ├── ServersScreen.tsx   # Liste des serveurs
│   │   ├── AddServerScreen.tsx # Ajout/édition serveur
│   │   ├── ReposScreen.tsx     # Liste des repos
│   │   ├── FeaturesScreen.tsx  # Liste des features
│   │   ├── TerminalScreen.tsx  # Terminal WebView
│   │   └── SettingsScreen.tsx  # Paramètres
│   ├── context/
│   │   ├── ThemeContext.tsx    # Thème dark/light
│   │   └── StorageContext.tsx  # Persistance locale
│   ├── utils/
│   │   ├── terminalHTML.ts     # HTML xterm.js
│   │   └── serverCommands.ts   # Communication serveur
│   └── types/
│       └── index.ts            # Types TypeScript
├── server-scripts/
│   ├── install.sh              # Installation serveur
│   ├── uninstall.sh            # Désinstallation serveur
│   ├── start-server.sh         # Démarrage ttyd+tmux
│   ├── list-repos.sh           # Liste des repos (JSON)
│   ├── list-features.sh        # Liste des features (JSON)
│   ├── create-feature.sh       # Création worktree
│   ├── delete-feature.sh       # Suppression worktree
│   └── switch-feature.sh       # Changement de feature
├── package.json
├── tsconfig.json
└── README.md
```

## ⚙️ Configuration

### Configuration Serveur (`~/.nomadflowcode/config.toml`)

```toml
[paths]
base_dir = "~/.nomadflowcode"

[tmux]
session = "nomadflow"

[ttyd]
port = 7681

[api]
port = 8080

# Authentification - décommenter pour activer
# Le même secret doit être entré dans l'app mobile
# [auth]
# secret = "votre-secret-ici"
```

### Configuration App (dans l'app)

- **Agent IA** : Claude, Ollama, ou commande personnalisée
- **Auto-lancement agent** : activer/désactiver
- **Préfixe session tmux** : personnalisable
- **Thème** : Dark, Light, ou Système
- **Taille police** : 10-24px
- **Reconnexion auto** : avec paramètres

## 🔐 Sécurité

### Authentification par Secret Partagé

NomadFlow utilise un secret partagé unique qui protège à la fois :
- **L'API REST** : via Bearer token (Authorization header)
- **Le terminal ttyd** : via Basic Auth (user: `nomadflow`, password: secret)

#### Activation

1. **Côté serveur** (`~/.nomadflowcode/config.toml`) :
```toml
[auth]
secret = "votre-secret-securise"
```

2. **Côté mobile** : entrez le même secret dans le champ "Secret d'authentification" lors de la configuration du serveur.

#### Fonctionnement

- **Sans secret** : tout fonctionne sans authentification (développement local)
- **Avec secret** : l'API retourne 401 sans le bon Bearer token, et ttyd demande les credentials

### Recommandations

1. **Utilisez HTTPS/WSS** en production
2. **Activez l'authentification** avec un secret fort
3. **Firewall** : n'exposez pas les ports 7681/8080 publiquement sans VPN
4. **Certificats SSL** : Let's Encrypt ou certificats auto-signés

## 🎮 Raccourcis tmux

L'app inclut des boutons overlay pour les raccourcis tmux courants :

| Raccourci | Action |
|-----------|--------|
| `Ctrl-b w` | Liste des windows |
| `Ctrl-b c` | Nouvelle window |
| `Ctrl-b n` | Window suivante |
| `Ctrl-b p` | Window précédente |
| `Ctrl-b "` | Split horizontal |
| `Ctrl-b %` | Split vertical |
| `Ctrl-b d` | Détacher |
| `Ctrl-b [` | Mode scroll |

## 🤝 Contribution

Les contributions sont les bienvenues !

1. Fork le projet
2. Créez votre branche (`git checkout -b feature/amazing-feature`)
3. Committez vos changements (`git commit -m 'Add amazing feature'`)
4. Pushez (`git push origin feature/amazing-feature`)
5. Ouvrez une Pull Request

## 📜 License

MIT License - voir [LICENSE](LICENSE) pour plus de détails.

## 🙏 Remerciements

- [ttyd](https://github.com/tsl0922/ttyd) - Terminal web
- [xterm.js](https://xtermjs.org/) - Émulateur de terminal
- [tmux](https://github.com/tmux/tmux) - Multiplexeur de terminal
- [React Native](https://reactnative.dev/) - Framework mobile

---

**Made with ❤️ for nomad developers**
