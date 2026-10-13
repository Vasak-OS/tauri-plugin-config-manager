# Tauri plugin config manager

Un simple plugin de tauri que permite leer y modificar la configuracion de VasakOS en tiempo real para poder integrarse con las preferencias del usuario

---

## Tecnología

[![Rust](https://img.shields.io/badge/Rust-1.82%2B-orange?style=flat&logo=rust)](https://www.rust-lang.org/) [![Tauri](https://img.shields.io/badge/Tauri-2.0-24283b?style=flat&logo=tauri)](https://tauri.app/) [![TypeScript](https://img.shields.io/badge/TypeScript-5.6-3178c6?style=flat&logo=typescript)](https://www.typescriptlang.org/)

## Estado

[![CI](https://github.com/Vasak-OS/tauri-plugin-config-manager/actions/workflows/ci.yml/badge.svg)](https://github.com/Vasak-OS/tauri-plugin-config-manager/actions/workflows/ci.yml)
[![Versión](https://img.shields.io/github/v/release/Vasak-OS/tauri-plugin-config-manager?style=flat&label=Versi%C3%B3n)](https://github.com/Vasak-OS/tauri-plugin-config-manager/releases)
[![Licencia](https://img.shields.io/github/license/Vasak-OS/tauri-plugin-config-manager?style=flat)](LICENSE)
[![Cobertura](https://sonarcloud.io/api/project_badges/measure?project=Vasak-OS_tauri-plugin-config-manager&metric=coverage&style=flat)](https://sonarcloud.io/summary/new_code?id=Vasak-OS_tauri-plugin-config-manager)
[![Quality Gate](https://sonarcloud.io/api/project_badges/measure?project=Vasak-OS_tauri-plugin-config-manager&metric=alert_status&style=flat)](https://sonarcloud.io/summary/new_code?id=Vasak-OS_tauri-plugin-config-manager)
[![Tamaño repo](https://img.shields.io/github/repo-size/Vasak-OS/tauri-plugin-config-manager?style=flat)](https://github.com/Vasak-OS/tauri-plugin-config-manager)

---

## Descripción

Un simple plugin de tauri que permite leer y modificar la configuracion de VasakOS en tiempo real para poder integrarse con las preferencias del usuario

**Para el usuario:** Un simple plugin de tauri que permite leer y modificar la configuracion de VasakOS en tiempo real para poder integrarse con las preferencias del usuario

**Integración VasakOS:** Integra con el ecosistema VasakOS vía D-Bus y configuración compartida.

---

## Arquitectura (diagramas)

> Copia y pega la plantilla que necesites, descomenta y adapta.

### Flujo de datos

```mermaid
graph TD
    A[Entrada: Teclado / Ratón / D-Bus / Sistema] --> B[Procesamiento: Core Rust + Tauri Commands]
    B --> C[Salida: UI actualizada / Acciones del sistema]
    style A fill:#e1f5fe
    style C fill:#e8f5e9
```

### Componentes internos

```mermaid
flowchart LR
    subgraph App[Tauri plugin config manager]
        UI[JS Bindings (TypeScript)]
        Core[Rust backend: plugin Tauri]
        IPC[Tauri invoke / events]
    end
    UI --> IPC
    IPC --> Core
    Core --> DBus[(D-Bus / zbus)]
    Core --> System[Tauri runtime + APIs nativas]
    style UI fill:#f3e5f5
    style Core fill:#e3f2fd
    style DBus fill:#fff3e0
    style System fill:#fce4ec
```

### Integración en el sistema VasakOS

```mermaid
C4Context
    title Tauri plugin config manager en VasakOS
    Person(usuario, "Usuario", "Persona que usa el sistema")
    System_Boundary(c1, "VasakOS") {
        System(app, "Tauri plugin config manager", "Plugin Tauri para config-manager")
        System_Ext(tauri, "Tauri v2", "Runtime de la app host")
    }
    System_Ext(systemd, "systemd --user", "Si el plugin usa servicios")
    System_Ext(dbus, "D-Bus", "Si expone interfaces")
    Rel(usuario, app, "Usa")
    Rel(app, tauri, "Se registra como plugin")
    Rel(app, systemd, "Opcional: servicios del plugin")
```

---

## Instalación

### Desde npm (recomendado)

```bash
# Con bun (recomendado para proyectos VasakOS)
bun add @vasakgroup/tauri-plugin-config-manager

# Con npm
npm install @vasakgroup/tauri-plugin-config-manager

# Con pnpm
pnpm add @vasakgroup/tauri-plugin-config-manager
```

### Desde fuente (desarrollo del plugin)

```bash
git clone https://github.com/Vasak-OS/tauri-plugin-config-manager.git
cd tauri-plugin-config-manager

# Build Rust
cargo build --release --locked

# Tests
cargo test --locked
bun test  # si tiene tests JS
```

---

## Uso rápido

```typescript
import { config_manager } from '@vasakgroup/tauri-plugin-config-manager';

// En tu app Tauri (tauri.conf.json → plugins):
// { "plugins": { "config-manager": { ...config... } } }

// Uso programático:
const plugin = config_manager();
const result = await plugin.someMethod();
```

### Atajos / flags principales

| Acción | Código |
|--------|--------|
| Instalar | `bun add @vasakgroup/tauri-plugin-config-manager` |
| Importar | `import { config_manager } from '@vasakgroup/tauri-plugin-config-manager'` |
| Configurar | `tauri.conf.json → plugins.config-manager` |

> **Captura:** `assets/demo.gif` *(placeholder — sustituir por GIF real)*

---

## Configuración

### Configuración en `tauri.conf.json`

```json
{
  "plugins": {
    "config-manager": {
      // Opciones específicas del plugin
    }
  }
}
```

### Variables de entorno (si aplica)

| Variable | Descripción | Por defecto |
|----------|-------------|-------------|
| `TAURI-PLUGIN-CONFIG-MANAGER_DEBUG` | Logs de depuración del plugin | `false` |

### Variables de entorno

| Variable | Descripción | Por defecto |
|----------|-------------|-------------|
| `VASAK_DEBUG` | Activa logs de depuración en consola | `false` |
| `VASAK_PROFILE` | Perfil de rendimiento: balanced, performance, power-save | `balanced` |

### Integración con `tauri-plugin-config-manager`

(plugin — la app host usa config-manager)

---

## Desarrollo

### Requisitos

- **Rust:** `rustup` toolchain estable (`rust-toolchain.toml` fija la versión)
- **Node / bun:** `bun` ≥ 1.1 (nunca npm/yarn/pnpm para apps Tauri)
- **Sistema:** `pkg-config`, `libgtk-3-dev`, `libwebkit2gtk-4.1-dev`, `libssl-dev`
- *`libssl-dev` para TLS, dependencias nativas específicas del plugin*

### Dependencias de desarrollo

#### Sistema (paquetes Arch / Debian / Fedora)

`base-devel`, `pkgconf`, `libssl-dev`, `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`, `gstreamer1.0-plugins-base`, `gstreamer1.0-plugins-good`, `libpipewire-0.3-dev`, `libwireplumber-0.5-dev`, `libdbus-1-dev`, `libudev-dev`, `libxkbcommon-dev`, `libwayland-dev`, `meson`, `ninja`, `clang`, `lld`

#### Rust

`rustup` → `stable` + `rustfmt` + `clippy`

#### Node

**Solo `bun`** (nunca npm/yarn/pnpm para apps Tauri). `bun --version` ≥ 1.1

#### Herramientas CLI

`just` (command runner), `cargo-binstall` (opcional), `cargo-llvm-cov` (para cobertura)

### Correr en desarrollo

```bash
# Build Rust en watch mode
cargo watch -x "build --lib"

# Tests
cargo test --locked
bun test  # si tiene bindings JS
```

### Compilar / Build de release

```bash
# Build release
cargo build --release --locked

# Publicar (requiere permisos en crates.io y npm)
cargo publish --locked
npm publish
```

### Targets principales

```bash
# Desarrollo
cargo watch -x "build --lib"

# Build producción
cargo build --release --locked

# Solo frontend
bun test  # tests JS bindings

# Solo backend Rust
cargo build --release --locked

# Lint + tipos (pre-commit / CI)
cargo fmt --check
cargo clippy -- -D warnings
bunx --bun biome check .  # si hay código JS
```

### Pruebas

```bash
# Tests Rust
cargo test -p tauri-plugin-config-manager --locked

# Tests JS (bindings)
bun test
```

### Convención de commits

[Conventional Commits](https://www.conventionalcommits.org/es/v1.0.0/):

```
<tipo>(<ámbito>): <descripción corta>

[cuerpo opcional]

[pie opcional: Closes #N]
```

Tipos habituales: `feat`, `fix`, `bugfix`, `docs`, `ci`, `chore`, `security`, `refactor`, `perf`, `test`.

### Cómo abrir PR

1. Rama desde `origin/main`:
   ```bash
   git worktree add -b feat/{{TEMA}} ~/wt/tauri-plugin-config-manager-{{TEMA}} origin/main
   ```
2. Prefijos de rama: `fix/`, `feat/`, `bugfix/`, `docs/`, `ci/`, `chore/`, `security/`
3. Escribe el fix → **`qa` escribe las pruebas** (fallan sin el fix) → suite verde → abre PR
4. Al abrir PR, **cuatro cosas obligatorias**:
   - Asignado a `JoaquinDecima`
   - Etiqueta `enhancement` o `bug` (+ `ci` / `dependencies` si aplica)
   - Añadido al proyecto «VasakOS Roadmap»
   - Status **In Progress**

---

## Arquitectura (texto)

### Crates / módulos principales

| Crate / Módulo | Responsabilidad |
|----------------|-----------------|
| `tauri-plugin-config-manager` | Plugin Tauri: Rust backend + JS bindings |

### Interfaces D-Bus / zbus expuestas



### Puntos de extensión

- **Plugins:** Sistema de plugins modular
- **Hooks:** Hooks de ciclo de vida
- **API pública Rust:** API pública del crate principal

### Features / Cargo features

| Feature | Descripción | Default |
|---------|-------------|---------|
| `async` | API asíncrona (futures) | ✓ |
| `blocking` | API bloqueante para compatibilidad | |



---

## Pruebas y calidad

- **Cobertura objetivo:** ≥ 80 % del código nuevo (SonarCloud Quality Gate)
- **Informes:** GitHub Actions → pestaña *SonarCloud Code Analysis* → enlace a SonarCloud
- **Añadir pruebas:** ver `AGENTS.md` sección 9 (mapa de repos y comandos de prueba)
- **Regla:** toda corrección lleva prueba escrita por `qa` que falla sin el fix

---

## Contribuir

- **Guía:** [`.github/CONTRIBUTING.md`](https://github.com/Vasak-OS/.github/blob/main/CONTRIBUTING.md)
- **Plantillas de issue:** [`.github/ISSUE_TEMPLATE/`](https://github.com/Vasak-OS/.github/tree/main/ISSUE_TEMPLATE)
- **Código de conducta:** [`CODE_OF_CONDUCT.md`](https://github.com/Vasak-OS/.github/blob/main/CODE_OF_CONDUCT.md)

---

## Contribuidores

> **Actualizado automáticamente en CI** — muestra los avatares de quienes han aportado código a este repo.

<div align="center">
  <a href="https://github.com/JoaquinDecima" target="_blank"><img src="https://avatars.githubusercontent.com/u/11674229?v=4&s=40" width="40" height="40" alt="JoaquinDecima" style="border-radius: 50%; margin: 2px;" title="JoaquinDecima (150 contribuciones)"></a>
</div>

[Ver todos los contribuidores](https://github.com/Vasak-OS/tauri-plugin-config-manager/graphs/contributors)

---

## Donadores / Sponsors

[![Sponsors](https://img.shields.io/github/sponsors/Vasak-OS?style=flat&label=Sponsors)](https://github.com/sponsors/Vasak-OS)

> **Backers:** *Actualizado automáticamente en CI desde GitHub Sponsors / OpenCollective / Polar.*

[💖 Donar al proyecto](https://github.com/sponsors/Vasak-OS)

---

## Licencia

`GPL-3.0` — Copyright © 2026 **Vasak-OS Contributors**

[Ver licencia completa](LICENSE)
