# Encore — diseño general y fase 1

Fecha: 2026-11-12
Estado: borrador para aprobar

## 1. Intención

Encore es un reproductor de escritorio para YouTube Music escrito en Rust nativo con iced. Tiene
dos objetivos, los dos igual de importantes:

1. **Reemplazar a Pear Desktop** (la app Electron que el usuario usa hoy) con una app nativa que
   se vea como Cider (el cliente alternativo de Apple Music) y que, a la larga, haga todo lo que
   hacen Cider y Pear, pero para YouTube Music.
2. **Aprender Rust desde cero.** El usuario no escribió nunca Rust ni iced; leyó el curso
   "Rust语言圣经" (https://beatai.org/rust-course) hasta el capítulo 所有权和借用 (ownership y
   borrowing). Encore es el proyecto con el que aprende.

Por el segundo objetivo, **el usuario escribe todo el código**. El asistente explica, guía, revisa
y commitea, pero no escribe el código de la app.

## 2. Objetivos del producto (todas las fases)

Lista de referencia armada a partir de Cider (cider.sh) y de los plugins de Pear
(`pear-devs/pear-desktop`, carpeta `src/plugins`):

- **Interfaz (de Cider):** inicio con recomendados y recientes, páginas de álbum y artista, letras
  sincronizadas en panel lateral (con traducción y romanización), modo inmersivo y karaoke,
  minirreproductor, temas y personalización.
- **Audio:** ecualizador de 10 bandas, velocidad de reproducción, crossfade, compresor, saltear
  silencios, volumen preciso y exponencial, elegir la salida de audio.
- **Integraciones:** controles multimedia del sistema (MPRIS en Linux), notificaciones, Discord
  Rich Presence, scrobbling a Last.fm y control remoto por una API local.
- **Específico de YouTube (de Pear):** SponsorBlock, saltear temas con dislike, alternar entre
  video y audio, elegir calidad, descargas, desactivar la reproducción automática, colores tomados
  de la carátula, visualizador y escuchar en grupo.
- **Extensibilidad:** sistema de plugins.

### Orden de fases (acordado)

1. **Esqueleto visual:** la pantalla principal en iced con datos de mentira. *(este documento)*
2. **Que suene un tema:** buscar en YouTube Music y reproducir el resultado.
3. **Tu cuenta:** login, biblioteca y playlists.
4. **Letras:** el panel lateral con datos reales y el modo inmersivo.
5. **Integraciones:** MPRIS, Discord y Last.fm.
6. **Extras de audio:** ecualizador, crossfade y el resto.
7. **Temas y plugins.**

Cada fase tiene su propio documento de diseño cuando le toque. Las fuentes de datos previstas para
las fases 2 en adelante (no son parte de la fase 1) son: la API interna de YouTube Music (InnerTube),
probablemente vía el crate `rustypipe` con `yt-dlp` como plan B para los streams; un motor de audio
(mpv o GStreamer, a decidir en la fase 2); y LRCLIB para las letras sincronizadas.

## 3. No objetivos y anti-objetivos

**No objetivos:**
- AirPlay, Apple Music Sing y la espacialización de audio propia de Cider (no se trasladan a
  YouTube Music).
- Envolver la web de YouTube Music en una webview, como hacen Pear y Tauri. Encore es nativo.
- Windows y macOS: no se bloquean a propósito, pero tampoco se prueban. *(decisión del asistente)*

**Anti-objetivos** (cosas que hacen fracasar el proyecto aunque la app ande):
- Que el asistente escriba el código de la app o que el usuario pegue código sin entenderlo.
- Usar "YouTube" en el nombre o en la marca de la app.
- Una interfaz que no se parezca a la maqueta aprobada.

## 4. Restricciones

- **Plataforma del usuario:** Arch Linux (CachyOS), compositor niri sobre Wayland, Rust 1.99
  instalado con pacman (sin rustup).
- **Repo:** `~/Projects/Encore`, remoto privado `YX14ng/Encore`, rama `main`.
- **Commits:** formato `<tipo>: <descripción>` en español (`feat`, `fix`, `refactor`, `docs`,
  `test`, `chore`, `perf`, `ci`), un tipo de cambio por commit. Nunca se commitea con la
  verificación en rojo.
- **Archivos chicos:** 200 a 400 líneas típico, 800 como tope; organizados por funcionalidad, no por
  tipo.
- **Idioma:** las explicaciones y la interfaz de la app, en español rioplatense. Los identificadores
  del código, en inglés.

## 5. Forma de trabajo (acordada con el usuario)

Cada paso de la guía tiene cinco partes, en este orden:

1. **Explicación:** qué concepto de Rust o de iced aparece en este paso y por qué hace falta, con un
   ejemplo chico que no sea el código de Encore.
2. **Lectura:** el capítulo exacto del curso Rust语言圣经 que cubre ese concepto, y nada más.
3. **Objetivo y pistas:** qué tiene que lograr el paso (por ejemplo, "un botón que diga Play y que
   al tocarlo diga Pausa"), con pistas, sin el código hecho.
4. **El usuario escribe y corre** el código con `cargo run`.
5. **Revisión:** el asistente revisa el código y lo que se ve, explica los errores del compilador y
   su causa y, cuando el paso está bien y la verificación pasa, lo commitea.

Los pasos arrancan muy chicos y se agrandan a medida que el usuario agarra confianza. Un paso
introduce como mucho un concepto nuevo importante.

**Arranque directo con iced (decisión del usuario).** No hay ejercicios previos de Rust puro: se va
directo a la ventana. Como una ventana vacía de iced ya usa structs, enums, `impl`, `match`,
closures, crates externos y más adelante genéricos y lifetimes (`Element<'_, Message>`), el código
que el usuario todavía no puede entender se marca explícitamente como "usalo ahora, lo entendés en
el paso N", y el paso N existe en el plan y lo explica. La explicación previa de cada paso dice qué
piezas son de este tipo. Ninguna pieza queda sin paso que la explique.

**El plan de pasos** va en el repo (`docs/superpowers/plans/`) con explicación, lectura, objetivo y
pistas de cada paso, **sin soluciones**. Las soluciones de referencia que usa el asistente para
revisar viven fuera del repo. *(decisión del asistente)*

**Archivos que no son código** (íconos SVG, la fuente Inter, las carátulas de prueba y los textos
de ejemplo: títulos, artistas, playlists y letras) los provee el asistente, porque tipearlos no
enseña Rust. El usuario escribe las estructuras y el código que los usan. *(decisión del
asistente)*

## 6. Fase 1: esqueleto visual

### 6.1 Objetivo

La ventana principal de Encore en iced, igual a la maqueta aprobada
(https://claude.ai/artifact/XesHz5mLWNanJGaGLLvSUP), con datos de mentira escritos en el código.
No hay red, ni YouTube, ni audio.

### 6.2 Qué se ve

Una ventana con tres columnas:

- **Barra lateral (izquierda, unos 236 px):** un buscador (decorativo en esta fase) y tres secciones
  plegables:
  - *YouTube Music:* Inicio, Explorar, Radios y mixes.
  - *Biblioteca:* Historial, Canciones, Álbumes, Artistas, Suscripciones, Podcasts.
  - *Playlists:* Nueva playlist, Música que me gusta y cinco playlists de ejemplo con un cuadradito
    de color.
  Cada entrada tiene un ícono del color de acento. La entrada elegida se resalta.
- **Centro:**
  - *Barra superior:* atrás, adelante, mostrar u ocultar la barra lateral y, a la derecha, escuchar
    juntos, transmitir, letras (activo) y cola. Solo el de la barra lateral y el de letras hacen
    algo en esta fase.
  - *Inicio (con scroll):* una imagen de fondo grande con el saludo encima ("Buenos días", "Buenas
    tardes" o "Buenas noches" según la hora local) y un degradado a oscuro abajo. Debajo, "Escuchado
    recientemente", una grilla de 3 columnas con 6 filas de ejemplo (carátula, título, artista y
    botón `···`), y "Mixes para vos", una fila horizontal de 5 tarjetas cuadradas.
  - *Reproductor (fijo abajo):* barra de progreso con tiempo transcurrido y restante; aleatorio,
    anterior, play/pausa, siguiente, repetir y like; una tarjeta con la carátula, el título, el
    artista y el formato del audio ("OPUS 160"); volumen, minirreproductor y pantalla completa.
- **Panel de letras (derecha, unos 300 px):** letras de ejemplo con tiempos. La línea actual se ve
  en blanco y más grande; las demás, atenuadas. La lista se desplaza sola para mantener la línea
  actual cerca del tercio superior, con fundido arriba y abajo.

**Fondo del inicio:** la carátula del tema que suena, difuminada (decisión final del usuario, que
pasó por "imagen elegida" y volvió a esta opción). Encore carga la carátula, la achica, le aplica un
desenfoque fuerte con el crate `image` una sola vez por tema y dibuja el resultado más grande que el
área del inicio, con un degradado a oscuro abajo para que se lea el saludo. Un desplazamiento o
zoom lento le da movimiento; si la versión de iced lo complica, una imagen quieta es aceptable como
primera versión.

**Carátulas de ejemplo:** tres o cuatro imágenes cuadradas chicas guardadas en `assets/covers/`,
provistas por el asistente. Sirven para las filas, los mixes, la tarjeta del reproductor y como
fuente del fondo difuminado. Para ver que el fondo cambia con el tema, el botón "siguiente" pasa al
próximo tema de ejemplo (la única excepción a "siguiente no hace nada" de la sección 6.3).

**Referencia visual:** la maqueta es la referencia de aspecto, pero donde este documento dice otra
cosa, manda el documento. Quedan fuera de la fase 1 el selector de fondo de la maqueta, los botones
de ventana propios y las adaptaciones a ventana angosta que no figuran en la sección 6.3. El botón
`···` de la barra superior se dibuja sin función. El estilo de letras de la maqueta (líneas pasadas
más tenues, leve desenfoque y línea actual agrandada) es opcional. *(decisión del asistente)*

**Estética:** solo tema oscuro, casi negro, con acento rojo rosado; tipografía Inter (empaquetada
con la app) con la fuente del sistema como respaldo. Los colores salen de los tokens de la maqueta:

| Token | Valor | Uso |
|---|---|---|
| fondo de página | `#0a0a0c` | detrás de todo |
| barra lateral | `#131315` | |
| centro | `#19191c` | |
| panel de letras | `#151517` | |
| texto | `#f3f3f5` | |
| texto secundario | `#9b9ba4` | |
| texto tenue | `#5f5f68` | |
| acento | `#ff4459` | íconos, like, `···` |
| líneas | blanco al 7 % | bordes y separadores |

### 6.3 Qué responde

- **Play/pausa:** arranca y frena un reloj falso que avanza un segundo por segundo. Con ese reloj
  avanzan la barra de progreso, los tiempos y la línea actual de las letras. Al llegar al final, el
  tema de mentira vuelve a empezar.
- **Like:** alterna el color del pulgar.
- **Mostrar u ocultar** la barra lateral y el panel de letras desde la barra superior.
- **Plegar y desplegar** cada sección de la barra lateral.
- **Elegir una entrada** de la barra lateral la resalta. Todavía no cambia de pantalla.
- **Ventana angosta:** por debajo de unos 1100 px de ancho se oculta el panel de letras; por debajo de
  unos 720 px, también la barra lateral.

- **Siguiente:** pasa al próximo tema de ejemplo (cambia carátula, fondo, título y reinicia el
  reloj).

Todo lo demás (buscador, atrás y adelante, anterior, cola, transmitir, volumen, minirreproductor y
pantalla completa) se dibuja pero no hace nada.

### 6.4 Estructura del código

Organizada por funcionalidad, con archivos chicos. Es una guía; los nombres exactos se ajustan
durante los pasos:

```
src/
  main.rs          arranque de iced y configuración de la ventana
  app.rs           estado global, enum Message, update, view y subscription (el reloj)
  theme.rs         colores, tipografías y estilos compartidos
  mock.rs          datos de mentira: tema actual, recientes, mixes, playlists y letras
  sidebar.rs       barra lateral
  home.rs          inicio: fondo difuminado, saludo, recientes y mixes
  player.rs        reproductor de abajo
  lyrics.rs        panel de letras y cálculo de la línea actual
assets/
  fonts/           Inter
  icons/           íconos en SVG
  covers/          carátulas de prueba (también fuente del fondo difuminado)
```

La lógica pura vive separada de la vista para poder testearla sin abrir ventanas: formatear
`93` como `1:33`, calcular el tiempo restante, elegir la línea de letras para un segundo dado y el
saludo según la hora.

### 6.5 Verificación

- `scripts/check.sh` corre `cargo fmt --check`, `cargo clippy -- -D warnings` y `cargo test`. Lo
  escribe el usuario en uno de los primeros pasos y se corre antes de cada commit.
- Tests unitarios para la lógica pura de la sección 6.4.
- La parte visual se verifica corriendo la app y comparándola con la maqueta. El asistente no abre
  ventanas en la PC del usuario (en otra pantalla corre un bot): el usuario la corre y manda una
  captura.

### 6.6 Criterio de terminado

- La ventana se parece a la maqueta en las tres columnas, con los colores de la tabla.
- Todo lo de la sección 6.3 funciona.
- `scripts/check.sh` pasa.
- El usuario puede explicar con sus palabras qué hacen `Message`, `update`, `view` y la subscription
  del reloj.

## 7. Lo que queda en manos de quien arme el plan

- La versión exacta de iced: la última estable al momento de arrancar, con las features que hagan
  falta (imágenes, SVG, tokio para el reloj).
- Cómo animar el desplazamiento de las letras dentro de lo que ofrece esa versión de iced (un salto
  sin animación es aceptable como primera versión).
- Las decoraciones de la ventana: en la fase 1 se usan las del sistema; dibujar botones propios de
  minimizar, maximizar y cerrar como Cider queda como opcional. *(decisión del asistente)*
- El orden y el tamaño de los pasos de la guía, respetando la sección 5 y empezando por: hola mundo
  con `cargo new`, ventana vacía de iced, botón Play/Pausa, las tres columnas como bloques de color
  y después llenarlas.
