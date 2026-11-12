# Encore fase 1 — guía de pasos

> **Cómo se usa esta guía:** la escribe el asistente y la sigue el usuario, que escribe todo el
> código. Cada paso tiene **Explicación** (el resumen; la explicación completa la da el asistente en
> el chat antes de arrancar), **Lectura**, **Objetivo**, **Pistas**, **Verificación** y **Commit**.
> No hay soluciones: las que usa el asistente para revisar viven fuera del repo. Los pasos marcados
> **(asistente)** los hace el asistente porque no son código (archivos de ejemplo, íconos y fuentes).
> Cada casilla `- [ ]` se tilda cuando el asistente revisó el paso y se commiteó.

**Objetivo:** la ventana principal de Encore en iced, igual a la maqueta, con datos de mentira.

**Arquitectura:** una app iced con el patrón estado → `Message` → `update` → `view`, más una
subscription para el reloj. La lógica pura (formatos, reloj, letras, saludo, layout) vive en
funciones chicas con tests; la vista se reparte en un archivo por zona de la pantalla.

**Stack:** Rust 1.99 (edición 2024), iced 0.14 con las features `tokio`, `image` y `svg`, crate
`image` 0.25 para el desenfoque y crate `chrono` para la hora local.

**Spec:** `docs/superpowers/specs/2026-11-12-encore-fase1-design.md`

**Lecturas:** todas son del curso Rust语言圣经. La ruta `basic/compound-type/struct` corresponde a
`https://beatai.org/rust-course/basic/compound-type/struct`; el nombre en chino es el del índice del
curso.

## Restricciones globales

- El usuario escribe todo el código de `src/` y `scripts/`; el asistente explica, revisa y commitea.
- Código que todavía no se entiende se marca con un comentario `// paso N` y el paso N lo explica.
- Identificadores en inglés; textos de la interfaz en español rioplatense.
- Archivos de 200 a 400 líneas, 800 como tope, organizados por zona de la pantalla.
- Commits `<tipo>: <descripción>` en español, un tipo por commit, solo con `scripts/check.sh` en
  verde (desde el paso 2).
- Colores: página `#0a0a0c`, barra lateral `#131315`, centro `#19191c`, letras `#151517`, texto
  `#f3f3f5`, secundario `#9b9ba4`, tenue `#5f5f68`, acento `#ff4459`, líneas blanco al 7 %.
- Anchos: barra lateral 236 px, panel de letras 300 px; se oculta letras por debajo de 1100 px de
  ventana y la barra lateral por debajo de 720 px.
- El asistente no abre ventanas en la PC: el usuario corre la app y manda captura.

## Foco de revisión

Fallas que el spec no nombra pero que un usuario se encontraría; cada una tiene su test en el paso
que la origina:

1. **Archivo de letras desordenado o con líneas raras** (metadatos `[ar:...]`, líneas vacías, tiempos
   mal escritos): se ignoran las inválidas, se ordenan por tiempo y no hay panic (paso 15).
2. **Tema sin letras:** el panel muestra "No hay letras para este tema" y nada indexa fuera de rango
   (paso 15).
3. **Tiempo transcurrido mayor que la duración** (al cambiar de tema o por un tick de más): el tiempo
   restante no da negativo ni hace overflow; da `0:00` (pasos 5 y 6).
4. **Carátula que no se puede decodificar:** el fondo cae a un color liso en vez de cerrar la app
   (paso 12).
5. **Ventana redimensionada y botones de ocultar combinados:** si la ventana es angosta el panel se
   oculta aunque el botón diga "mostrar", y vuelve si se agranda (paso 16).

---

### Paso 1: hola mundo con Cargo

**Explicación:** qué es Cargo, para qué están `Cargo.toml` y `src/main.rs`, y la diferencia entre
`cargo build`, `cargo run` y `cargo check`. El repo ya existe, así que se usa `cargo init` en vez de
`cargo new`.

**Lectura:** `first-try/cargo` (认识 Cargo), `first-try/hello-world` (不仅仅是 Hello world).

**Objetivo:** `cargo run` en `~/Projects/Encore` imprime `Encore arrancando`.

**Pistas:**
- `cargo init --name encore` dentro del repo. Mirá qué agregó al `.gitignore` y si choca con el que
  ya había.
- Revisá que `Cargo.toml` diga `edition = "2024"`.

**Verificación:** `cargo run` imprime `Encore arrancando`.

- [ ] **Commit:** `chore: inicializar el proyecto con cargo`

### Paso 2: script de verificación

**Explicación:** qué hacen `cargo fmt`, `cargo clippy` y `cargo test`, y por qué un script corta en
el primer error (`set -euo pipefail`).

**Lectura:** ninguna del curso.

**Objetivo:** `scripts/check.sh` corre, en este orden, `cargo fmt --check`,
`cargo clippy --all-targets -- -D warnings` y `cargo test`, y termina con código distinto de 0 si
algo falla.

**Pistas:** primera línea `#!/usr/bin/env bash`; hacelo ejecutable con `chmod +x`. Probalo rompiendo
el formato a propósito (un espacio de más) y mirá que falle.

**Verificación:** `scripts/check.sh` termina sin errores sobre el hola mundo.

- [ ] **Commit:** `chore: agregar script de verificación`

### Paso 3: ventana vacía de iced

**Explicación:** qué es un crate externo y cómo se agrega; la idea general de iced (la app es un
estado, y iced llama a `view` para dibujarlo). Este paso usa piezas que todavía no leíste, todas
marcadas: `struct` y `impl` (paso 4), `enum Message` (paso 4), `#[derive(...)]` (paso 10),
`Element<'_, Message>` (paso 11).

**Lectura:** `basic/base-type/function` (函数, repaso).

**Objetivo:** `cargo run` abre una ventana de 1280×800 titulada "Encore" con el texto "Encore"
centrado.

**Pistas:**
- `cargo add iced@0.14 --features tokio,image,svg`.
- `main` devuelve `iced::Result`.
- `iced::application(App::default, App::update, App::view)` con `.title("Encore")`,
  `.window_size((1280.0, 800.0))` y `.run()`.
- `App` es un struct vacío con `#[derive(Default)]`; `Message` es un enum con una variante
  cualquiera por ahora.
- Firmas: `fn update(&mut self, message: Message)` y `fn view(&self) -> Element<'_, Message>`.
- Para centrar: `container(text("Encore")).center(Fill)`.

**Verificación:** la ventana abre (mandá captura) y `scripts/check.sh` pasa.

- [ ] **Commit:** `feat: abrir la ventana de Encore`

### Paso 4: botón Play/Pausa

**Explicación:** el ciclo de iced: el usuario toca algo → se genera un `Message` → `update` cambia el
estado → `view` lo vuelve a dibujar. Acá se explican de verdad `struct`, `impl`, `enum` y `match`
del paso 3.

**Lectura:** `basic/compound-type/struct` (结构体), `basic/compound-type/enum` (枚举),
`basic/match-pattern/match-if-let` (match 和 if let), `basic/method` (方法 Method).

**Objetivo:** un botón que dice "Play" y al tocarlo pasa a "Pausa", y al revés.

**Pistas:**
- `App` gana un campo `is_playing: bool`; `Message` tiene la variante `TogglePlay`.
- `button(texto).on_press(Message::TogglePlay)`.
- El texto sale de un `if` usado como expresión.

**Verificación:** el botón alterna (captura de los dos estados); `scripts/check.sh` pasa.

- [ ] **Commit:** `feat: botón de play y pausa`

### Paso 5: formatear tiempos, con tests

**Explicación:** módulos (`mod`, archivos, `pub`), tests unitarios (`#[cfg(test)]`, `#[test]`,
`assert_eq!`) y por qué la resta de enteros sin signo puede hacer overflow.

**Lectura:** `basic/crate-module/module` (模块 Module), `basic/formatted-output` (格式化输出),
`test/write-tests` (编写测试及控制执行).

**Objetivo:** en `src/clock.rs`, dos funciones puras:
- `pub fn format_time(secs: u32) -> String`
- `pub fn remaining(elapsed: u32, duration: u32) -> u32`

**Pistas:** `format!` con `{:02}` para los segundos; mirá `saturating_sub` para la resta.

**Tests que tenés que escribir** (en el mismo archivo):

| Llamada | Resultado |
|---|---|
| `format_time(0)` | `"0:00"` |
| `format_time(59)` | `"0:59"` |
| `format_time(93)` | `"1:33"` |
| `format_time(214)` | `"3:34"` |
| `format_time(600)` | `"10:00"` |
| `remaining(93, 214)` | `121` |
| `remaining(214, 214)` | `0` |
| `remaining(300, 214)` | `0` (foco de revisión 3) |

**Verificación:** primero escribí los tests y mirá que fallen (`cargo test`), después las funciones y
mirá que pasen; `scripts/check.sh` pasa.

- [ ] **Commit:** `feat: formato de tiempos del reproductor`

### Paso 6: reloj falso con subscription

**Explicación:** qué es una subscription (iced te manda mensajes solos, como un tick por segundo) y
qué es un closure, que acá aparece en `.map(|_| Message::Tick)`.

**Lectura:** `advance/functional-programing/closure` (闭包 Closure).

**Objetivo:** con Play, un texto `1:33 / -2:01` avanza un segundo por segundo; con Pausa se frena.
Al llegar a la duración, vuelve a `0:00`.

**Pistas:**
- En `src/clock.rs`, un struct `Playback { elapsed: u32, duration: u32, is_playing: bool }` con
  `pub fn tick(&mut self)` y `pub fn toggle(&mut self)`. `is_playing` se muda de `App` acá.
- La subscription: `iced::time::every(Duration::from_secs(1)).map(|_| Message::Tick)` si está
  sonando, `Subscription::none()` si no. Se registra con `.subscription(App::subscription)`.
- Arrancá con `elapsed: 93, duration: 214` para que coincida con la maqueta.

**Tests que tenés que escribir:**

| Estado inicial | Acción | `elapsed` después |
|---|---|---|
| 93 de 214, sonando | `tick()` | 94 |
| 213 de 214, sonando | `tick()` | 0 (vuelve a empezar) |
| 93 de 214, en pausa | `tick()` | 93 |
| 300 de 214, sonando | `tick()` | 0 (foco de revisión 3) |

**Verificación:** el contador avanza y se frena; `scripts/check.sh` pasa.

- [ ] **Commit:** `feat: reloj de reproducción simulado`

### Paso 7: colores y las tres columnas

**Explicación:** `const`, `use` entre módulos y cómo iced estiliza un `container` con un closure que
devuelve un `container::Style`.

**Lectura:** `basic/crate-module/use` (使用 use 引入模块及受限可见性).

**Objetivo:** la ventana se divide en tres columnas de los colores del spec: barra lateral de 236 px,
centro que ocupa el resto y panel de letras de 300 px. El botón y el reloj quedan en el centro.

**Pistas:**
- `src/theme.rs` con constantes `pub const SIDEBAR: Color = Color::from_rgb8(0x13, 0x13, 0x15);`,
  y así para cada color del spec. Las líneas: `Color::from_rgba8(255, 255, 255, 0.07)`.
- `row![a, b, c]`; cada uno es un `container(...).width(236).height(Fill).style(...)`.
- La app con fondo oscuro: `.theme(|_| Theme::Dark)` o un `style` propio sobre la app.

**Verificación:** captura con las tres columnas; `scripts/check.sh` pasa.

- [ ] **Commit:** `feat: estructura de tres columnas`

### Paso 8: barra superior y ocultar columnas

**Explicación:** `Option` como "hay o no hay algo", y cómo iced acepta un `Option` como hijo (si es
`None`, no dibuja nada). `bool::then`.

**Lectura:** `basic/match-pattern/option` (解构 Option).

**Objetivo:** el centro tiene arriba una barra con los botones `‹`, `›`, barra lateral, `···`,
escuchar juntos, transmitir, letras y cola (por ahora con texto, los íconos llegan en el paso 10).
Los de barra lateral y letras ocultan y muestran su columna.

**Pistas:**
- `App` gana `show_sidebar: bool` y `show_lyrics: bool`, que arrancan en `true`.
- `Message::ToggleSidebar` y `Message::ToggleLyrics`.
- `.push(self.show_sidebar.then(|| sidebar_view()))`.
- Separá la vista: `src/sidebar.rs`, `src/home.rs`, `src/player.rs` y `src/lyrics.rs`, cada uno con
  una función `pub fn view(...) -> Element<'_, Message>`.

**Verificación:** los dos botones ocultan y muestran su columna (captura); `scripts/check.sh` pasa.

- [ ] **Commit:** `feat: barra superior y columnas ocultables`

### Paso 9 (asistente): archivos de ejemplo

El asistente agrega:
- `assets/mock/tracks.tsv`: los temas de ejemplo, uno por línea, separados por tabulador:
  título, artista, álbum, duración en segundos, archivo de carátula y archivo de letras (vacío si no
  tiene).
- `assets/mock/recents.tsv` y `assets/mock/mixes.tsv`: título, subtítulo y carátula.
- `assets/mock/playlists.tsv`: nombre y color en hex.
- `assets/mock/luz-de-neon.lrc`: letras inventadas en formato LRC (`[mm:ss.cc] texto`), con un
  metadato `[ar:...]`, una línea vacía y una línea fuera de orden a propósito (foco de revisión 1).
- `assets/covers/*.png`: cinco carátulas de 300×300 generadas.

- [ ] **Commit (asistente):** `chore: agregar datos y carátulas de ejemplo`

### Paso 10: datos de ejemplo e íconos en la barra lateral

**Explicación:** `String` vs `&str`, `Vec`, iteradores (`lines`, `split`, `filter_map`) e
`include_str!` para meter un archivo dentro del ejecutable. También `#[derive(Debug, Clone, Copy,
PartialEq)]`, que quedó pendiente del paso 3.

**Lectura:** `basic/compound-type/string-slice` (字符串与切片), `basic/collections/vector`
(动态数组 Vector), `advance/functional-programing/iterator` (迭代器 Iterator), `basic/trait/trait`
(特征 Trait, solo la parte de `derive`).

**Objetivo:**
- `src/mock.rs` con `pub struct Track { title, artist, album, duration, cover, lyrics }` y
  `pub struct Card { title, subtitle, cover }` (todos los textos como `String`), más
  `pub fn parse_tracks(text: &str) -> Vec<Track>` y `pub fn parse_cards(text: &str) -> Vec<Card>`.
- La barra lateral completa: buscador (decorativo), tres secciones plegables y las playlists de
  `playlists.tsv` con su cuadradito de color. Elegir una entrada la resalta.

**Pistas:**
- Una línea que no tiene todos los campos se saltea (`filter_map` + `Option`).
- `enum Section { Music, Library, Playlists }` y `enum NavItem { Home, Explore, ... }`; el estado
  guarda qué secciones están abiertas y qué entrada está elegida.
- `Message::ToggleSection(Section)` y `Message::Select(NavItem)`.
- El asistente pasa los íconos en `assets/icons/` junto con este paso; se cargan con
  `svg(svg::Handle::from_memory(include_bytes!("../assets/icons/home.svg")))` y se pintan del color de
  acento con su `style`.

**Tests que tenés que escribir:**

| Entrada de `parse_tracks` | Resultado |
|---|---|
| una línea con los 6 campos | un `Track` con esos valores y `duration` como número |
| una línea con 5 campos | se saltea |
| una línea con la duración `abc` | se saltea |
| líneas vacías al final | se ignoran |
| el `tracks.tsv` real | 5 temas |

**Verificación:** captura de la barra lateral con una sección plegada; `scripts/check.sh` pasa.

- [ ] **Commit:** `feat: barra lateral con datos de ejemplo`

### Paso 11: inicio — saludo, recientes y mixes

**Explicación:** lifetimes, a partir de `Element<'_, Message>` del paso 3: la vista toma prestado el
estado, y el `'_` dice que lo dibujado no puede vivir más que ese préstamo. También `chunks` para
armar filas de a 3.

**Lectura:** `basic/lifetime` (认识生命周期).

**Objetivo:**
- `pub fn greeting(hour: u32) -> &'static str` en `src/home.rs`.
- El inicio con un área de 330 px de alto (todavía con fondo liso) y el saludo según la hora local.
- "Escuchado recientemente" en una grilla de 3 columnas con las 6 tarjetas de `recents.tsv`:
  carátula de 44 px, título, subtítulo y `···` en color de acento.
- "Mixes para vos": una fila horizontal con scroll de las tarjetas de `mixes.tsv`, carátulas de
  150 px.
- Todo el inicio dentro de un `scrollable` vertical.

**Pistas:** `cargo add chrono` y `chrono::Local::now().hour()` (necesita `use chrono::Timelike`). Las
carátulas, con `image(image::Handle::from_bytes(...))`; por ahora podés leerlas con `include_bytes!`
o con `Handle::from_path`.

**Tests que tenés que escribir:**

| `greeting(h)` | Resultado |
|---|---|
| 4 | `"Buenas noches"` |
| 5 | `"Buenos días"` |
| 11 | `"Buenos días"` |
| 12 | `"Buenas tardes"` |
| 19 | `"Buenas tardes"` |
| 20 | `"Buenas noches"` |
| 23 | `"Buenas noches"` |

**Verificación:** captura del inicio; `scripts/check.sh` pasa.

- [ ] **Commit:** `feat: inicio con saludo, recientes y mixes`

### Paso 12: fondo de carátula difuminada

**Explicación:** `Result`, el operador `?` y cuándo un error se maneja en vez de hacer panic.
Trabajar con píxeles: decodificar, achicar y desenfocar.

**Lectura:** `basic/result-error/result` (返回值 Result 和?), `basic/result-error/panic`
(panic! 深入剖析).

**Objetivo:** en `src/backdrop.rs`:
- `pub fn backdrop_size(width: u32, height: u32) -> (u32, u32)`: ancho máximo 480 sin agrandar,
  manteniendo la proporción.
- `pub fn blurred_backdrop(bytes: &[u8]) -> Option<image::Handle>`: decodifica, achica con
  `backdrop_size`, desenfoca y devuelve un handle; `None` si los bytes no son una imagen.
- El área de 330 px del inicio muestra la carátula del tema actual desenfocada, recortada para
  llenar (`ContentFit::Cover`), con un degradado a `#19191c` abajo y el saludo encima. Si
  `blurred_backdrop` da `None`, se ve el color del centro.
- El fondo se calcula una vez por tema y se guarda en el estado, no en cada `view`.
- Opcional: un desplazamiento o zoom lento del fondo con el tick del reloj.

**Pistas:** `cargo add image@0.25 --no-default-features --features png`; `image::load_from_memory`,
`resize`, `imageops::fast_blur` (probá un sigma entre 20 y 40), `to_rgba8` y
`iced::widget::image::Handle::from_rgba`. Para apilar: `stack![fondo, degradado, contenido]`; el
degradado es un `container` con `Background::Gradient`.

**Tests que tenés que escribir:**

| Llamada | Resultado |
|---|---|
| `backdrop_size(1200, 1200)` | `(480, 480)` |
| `backdrop_size(960, 480)` | `(480, 240)` |
| `backdrop_size(300, 300)` | `(300, 300)` |
| `blurred_backdrop(b"esto no es una imagen")` | `None` (foco de revisión 4) |
| `blurred_backdrop` con una carátula de `assets/covers/` | `Some(_)` |

**Verificación:** captura del inicio con el fondo; `scripts/check.sh` pasa.

- [ ] **Commit:** `feat: fondo de carátula difuminada en el inicio`

### Paso 13: reproductor de abajo

**Explicación:** widgets con valor (`progress_bar`), alinear filas y reutilizar una función que
arma botones con ícono.

**Lectura:** `basic/trait/generic` (泛型 Generics, para entender las firmas de iced; no hace falta
escribir genéricos).

**Objetivo:** el reproductor fijo abajo del centro, como en la maqueta: barra de progreso con
`format_time` a la izquierda y `-` restante a la derecha; aleatorio, anterior, play/pausa,
siguiente, repetir y like; la tarjeta del tema con carátula de 40 px, título, artista y la etiqueta
`OPUS 160`; volumen, minirreproductor y pantalla completa.

**Pistas:**
- `progress_bar(0.0..=duration as f32, elapsed as f32)` con altura 4 y estilo propio.
- `App` gana `liked: bool` y `Message::ToggleLike`; el pulgar va en acento si está activo.
- `Playback` pasa a saber qué tema suena (`track: usize`) y toma la duración de `Track`.
- Botones sin función: `button(...)` sin `on_press` se dibuja desactivado; dales un estilo que no se
  vea gris.

**Verificación:** captura del reproductor; play, pausa y like andan; `scripts/check.sh` pasa.

- [ ] **Commit:** `feat: reproductor con progreso y controles`

### Paso 14: siguiente tema

**Explicación:** el resto de la división (`%`) para dar la vuelta a una lista, y actualizar varias
partes del estado desde un solo mensaje.

**Lectura:** `basic/flow-control` (流程控制, repaso).

**Objetivo:** `Message::Next` pasa al próximo tema de `tracks.tsv`: cambia la tarjeta, la duración,
el fondo difuminado y reinicia el reloj a `0:00`. Después del último vuelve al primero.

**Pistas:** `pub fn next_index(current: usize, len: usize) -> usize` en `src/clock.rs`. El fondo se
recalcula en `update`, no en `view`.

**Tests que tenés que escribir:**

| `next_index(current, len)` | Resultado |
|---|---|
| `(0, 5)` | `1` |
| `(4, 5)` | `0` |
| `(0, 1)` | `0` |

**Verificación:** apretando siguiente cambian la tarjeta y el fondo (dos capturas);
`scripts/check.sh` pasa.

- [ ] **Commit:** `feat: pasar al siguiente tema`

### Paso 15: panel de letras

**Explicación:** parsear texto con `split_once`, `strip_prefix` y `parse`; `let ... else`; ordenar
un `Vec`; y mostrar una porción de la lista (`&lines[a..b]`) sin salirse del rango.

**Lectura:** `basic/match-pattern/pattern-match` (模式适用场景), repaso de `string-slice`.

**Objetivo:** en `src/lyrics.rs`:
- `pub struct LyricLine { pub start: u32, pub text: String }` (`start` en segundos).
- `pub fn parse_lrc(text: &str) -> Vec<LyricLine>`: ignora metadatos, líneas vacías y tiempos
  inválidos, y devuelve las líneas ordenadas por `start`.
- `pub fn current_line(lines: &[LyricLine], elapsed: u32) -> Option<usize>`.
- El panel muestra 2 líneas anteriores, la actual en `#f3f3f5` y algo más grande, y las 6
  siguientes atenuadas. Si el tema no tiene letras, "No hay letras para este tema".

**Pistas:** las letras se parsean una vez, al cambiar de tema. Para la porción, `saturating_sub` y
`min(len)`. El desplazamiento animado y el fundido arriba y abajo son opcionales.

**Tests que tenés que escribir:**

| Entrada | Resultado |
|---|---|
| `parse_lrc("[01:10.00] Las calles")` | una línea, `start` 70 |
| `parse_lrc("[ar:Los Satélites]")` | vacío |
| `parse_lrc("[xx:10.00] hola")` | vacío |
| `parse_lrc` con `[01:16.00] b` antes de `[01:10.00] a` | `a` primero (foco 1) |
| `current_line(&lineas, 69)` con la primera en 70 | `None` |
| `current_line(&lineas, 70)` | `Some(0)` |
| `current_line(&lineas, 76)` con la segunda en 76 | `Some(1)` |
| `current_line(&lineas, 999)` | el índice de la última |
| `current_line(&[], 50)` | `None` (foco 2) |

**Verificación:** las letras avanzan con el reloj y el tema sin letras muestra el mensaje
(capturas); `scripts/check.sh` pasa.

- [ ] **Commit:** `feat: panel de letras sincronizadas`

### Paso 16: tipografía, ventana angosta y retoques

**Explicación:** cargar una fuente con `.font(...)` y `.default_font(...)`; escuchar el tamaño de
la ventana con una segunda subscription (`Subscription::batch`).

**Lectura:** ninguna nueva; repaso de `closure`.

**Objetivo:**
- Inter como fuente de toda la app (el asistente agrega `assets/fonts/` con su licencia OFL).
- `pub fn visible_columns(width: f32, show_sidebar: bool, show_lyrics: bool) -> (bool, bool)` en
  `src/app.rs`: devuelve si se ven la barra lateral y las letras.
- Retoques contra la maqueta: bordes de 1 px en color de línea, esquinas, tamaños de texto y hover.

**Pistas:** `iced::window::resize_events()` da `(Id, Size)`; guardá el ancho en el estado. Los
botones de ocultar siguen mandando aunque la ventana sea ancha.

**Tests que tenés que escribir:**

| `visible_columns(w, sidebar, lyrics)` | Resultado |
|---|---|
| `(1280.0, true, true)` | `(true, true)` |
| `(1099.0, true, true)` | `(true, false)` |
| `(719.0, true, true)` | `(false, false)` |
| `(1280.0, false, true)` | `(false, true)` |
| `(1099.0, true, false)` | `(true, false)` |

**Verificación:** capturas a ancho completo y angosto; `scripts/check.sh` pasa.

- [ ] **Commit:** `feat: tipografía y ventana adaptable`

### Paso 17: cierre de la fase 1

**Objetivo:**
- Comparar la app con la maqueta zona por zona; anotar lo que falte y decidir si se arregla o pasa a
  otra fase.
- Recorrer el criterio de terminado del spec (sección 6.6). Incluye que el usuario explique con sus
  palabras qué hacen `Message`, `update`, `view` y la subscription del reloj.
- `scripts/check.sh` en verde y push a `origin/main`.

- [ ] **Push** al cerrar la fase.
