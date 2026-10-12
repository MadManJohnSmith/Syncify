<template>
  <!-- UI-CORR-1 (10-oct-2026): salir del stacking context del footer
       `StatusBar` (z-[50]/z-[160]) y del `main` con overflow-hidden. Un hijo
       z-[200] dentro de ese footer nunca podía cubrir al resto de la app;
       Teleport lo convierte en hijo directo de body y conserva los botones
       Cerrar/Esc y el estado de usePlayer sin duplicar reproductores. -->
  <Teleport to="body">
    <Transition name="fullscreen-fade">
    <div
      v-if="isFullscreenOpen"
      data-testid="fullscreen-player"
      class="fixed inset-0 z-[200] bg-shell/95 backdrop-blur-2xl flex flex-col text-ink select-none overflow-hidden"
    >
      <!-- Aura / ambient glow -->
      <div
        aria-hidden="true"
        class="absolute -top-40 -left-40 w-96 h-96 bg-accent/20 rounded-full blur-3xl pointer-events-none"
      ></div>
      <div
        aria-hidden="true"
        class="absolute -bottom-40 -right-40 w-96 h-96 bg-primary/20 rounded-full blur-3xl pointer-events-none"
      ></div>

      <!-- Top Header Bar -->
      <header class="relative z-10 flex items-center justify-between px-6 py-4 border-b border-line shrink-0">
        <div class="flex items-center gap-3">
          <span class="material-symbols-outlined text-accent text-2xl">album</span>
          <div>
            <h1 class="text-sm font-semibold tracking-wider uppercase text-ink">Reproductor Completo</h1>
            <p class="text-xs text-text-secondary">Syncify Hi-Fi Player</p>
          </div>
        </div>

        <!-- Center Tabs -->
        <nav class="flex items-center gap-1 bg-surface/80 p-1 rounded-xl border border-line">
          <button
            @click="activeTab = 'lyrics'"
            data-testid="tab-lyrics"
            :class="[
              'flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-all',
              activeTab === 'lyrics' ? 'bg-elevated text-ink shadow-sm' : 'text-text-secondary hover:text-ink'
            ]"
          >
            <span class="material-symbols-outlined text-sm">lyrics</span>
            <span>Letras</span>
          </button>

          <button
            @click="activeTab = 'metadata'"
            data-testid="tab-metadata"
            :class="[
              'flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-all',
              activeTab === 'metadata' ? 'bg-elevated text-ink shadow-sm' : 'text-text-secondary hover:text-ink'
            ]"
          >
            <span class="material-symbols-outlined text-sm">info</span>
            <span>Metadatos</span>
          </button>

          <button
            @click="activeTab = 'booklet'"
            data-testid="tab-booklet"
            :class="[
              'flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-all',
              activeTab === 'booklet' ? 'bg-elevated text-ink shadow-sm' : 'text-text-secondary hover:text-ink'
            ]"
          >
            <span class="material-symbols-outlined text-sm">menu_book</span>
            <span>Booklet</span>
            <span
              v-if="hasBooklet"
              class="w-1.5 h-1.5 rounded-full bg-accent"
              title="Booklet disponible"
            ></span>
          </button>

          <button
            @click="activeTab = 'queue'"
            data-testid="tab-queue"
            :class="[
              'flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-all',
              activeTab === 'queue' ? 'bg-elevated text-ink shadow-sm' : 'text-text-secondary hover:text-ink'
            ]"
          >
            <span class="material-symbols-outlined text-sm">queue_music</span>
            <span>Cola ({{ queue.length }})</span>
          </button>
        </nav>

        <!-- Right Window Actions -->
        <div class="flex items-center gap-2">
          <button
            @click="toggleNativeFullscreen"
            class="p-2 rounded-lg bg-surface hover:bg-elevated text-text-secondary hover:text-ink transition-colors"
            title="Pantalla completa de ventana"
            aria-label="Pantalla completa de ventana"
          >
            <span class="material-symbols-outlined text-lg">
              {{ isNativeFullscreen ? 'fullscreen_exit' : 'fullscreen' }}
            </span>
          </button>

          <button
            @click="closeFullscreen"
            data-testid="close-fullscreen-player"
            class="p-2 rounded-lg bg-surface hover:bg-elevated text-text-secondary hover:text-ink transition-colors"
            title="Cerrar reproductor (Esc)"
            aria-label="Cerrar reproductor"
          >
            <span class="material-symbols-outlined text-lg">close</span>
          </button>
        </div>
      </header>

      <!-- Main Body Container -->
      <main class="relative z-10 flex-1 grid grid-cols-1 lg:grid-cols-12 gap-8 p-6 lg:p-10 min-h-0 overflow-y-auto lg:overflow-hidden">
        <!-- Left Column: Artwork & Playback Transport -->
        <section class="lg:col-span-5 flex flex-col justify-between max-w-lg mx-auto w-full min-h-0 space-y-6">
          <!-- Artwork / Animated Cover Section (PL-04) -->
          <div class="flex flex-col items-center">
            <!-- Cover View Switcher (Animada vs Estática HD) -->
            <div
              v-if="hasAnimatedCover"
              class="flex items-center gap-2 mb-3 bg-surface/80 p-1 rounded-full border border-line text-xs"
            >
              <button
                @click="coverViewMode = 'animated'"
                data-testid="toggle-cover-animated"
                :class="[
                  'px-3 py-1 rounded-full flex items-center gap-1 transition-colors',
                  coverViewMode === 'animated' ? 'bg-accent text-white font-medium' : 'text-text-secondary hover:text-ink'
                ]"
              >
                <span class="material-symbols-outlined text-xs">motion_photos_on</span>
                <span>Animada</span>
              </button>
              <button
                @click="coverViewMode = 'hd'"
                data-testid="toggle-cover-hd"
                :class="[
                  'px-3 py-1 rounded-full flex items-center gap-1 transition-colors',
                  coverViewMode === 'hd' ? 'bg-accent text-white font-medium' : 'text-text-secondary hover:text-ink'
                ]"
              >
                <span class="material-symbols-outlined text-xs">image</span>
                <span>HD Estática</span>
              </button>
            </div>

            <!-- Cover Box -->
            <div
              data-testid="fullscreen-cover-container"
              class="relative w-64 h-64 sm:w-80 sm:h-80 md:w-96 md:h-96 rounded-2xl overflow-hidden shadow-2xl border border-line/60 bg-surface flex items-center justify-center group"
            >
              <!-- Animated Cover (WebP / Video) -->
              <img
                v-if="hasAnimatedCover && coverViewMode === 'animated'"
                :src="animatedCoverUrl!"
                alt="Portada animada"
                data-testid="animated-cover-img"
                class="w-full h-full object-cover transition-transform duration-500 group-hover:scale-105"
                @error="onAnimatedCoverError"
              />

              <!-- Static HD Cover / Fallback (TrackCover) -->
              <template v-else>
                <img
                  v-if="hdCoverUrl"
                  :src="hdCoverUrl"
                  :alt="current?.title ?? 'Portada HD'"
                  data-testid="hd-cover-img"
                  class="w-full h-full object-cover transition-transform duration-500 group-hover:scale-105"
                  @error="hdCoverUrl = null"
                />
                <TrackCover
                  v-else
                  :src="current?.coverUrl"
                  :item-id="current?.id"
                  :alt="current?.album ?? current?.title"
                  size-class="w-full h-full text-5xl"
                  rounded="rounded-2xl"
                />
              </template>

              <!-- Audio Quality Badge -->
              <div
                v-if="qualityBadge"
                class="absolute bottom-3 left-3 bg-black/75 backdrop-blur-md px-2.5 py-1 rounded-md text-[11px] font-mono font-medium text-emerald-400 border border-emerald-500/30 shadow-lg flex items-center gap-1"
                data-testid="audio-quality-badge"
              >
                <span class="material-symbols-outlined text-xs">graphic_eq</span>
                <span>{{ qualityBadge }}</span>
              </div>
            </div>
          </div>

          <!-- Track Information -->
          <div class="text-center space-y-1">
            <h2
              data-testid="fullscreen-title"
              class="text-xl sm:text-2xl font-bold text-ink truncate px-2"
              :title="current?.title"
            >
              {{ current?.title ?? 'Sin reproducción activa' }}
            </h2>
            <p
              data-testid="fullscreen-artist"
              class="text-base text-text-secondary truncate px-2"
            >
              {{ current?.artist ?? '—' }}
            </p>
            <p
              v-if="current?.album"
              data-testid="fullscreen-album"
              class="text-xs text-ink-3 truncate px-2"
            >
              {{ current?.album }}
            </p>
          </div>

          <!-- Transport Controls -->
          <div class="space-y-4">
            <!-- Progress Bar / Seek Slider -->
            <div class="space-y-1">
              <div class="relative flex items-center">
                <input
                  type="range"
                  min="0"
                  :max="durationSec || 0"
                  step="0.1"
                  :value="positionSec"
                  @input="seek(Number(($event.target as HTMLInputElement).value))"
                  :disabled="!durationSec"
                  data-testid="fullscreen-seek"
                  class="w-full h-2 rounded-lg bg-surface appearance-none cursor-pointer accent-accent"
                  aria-label="Posición de reproducción"
                />
              </div>
              <div class="flex items-center justify-between text-xs font-mono text-ink-3 tabular-nums px-0.5">
                <span>{{ formatClockTime(positionSec) }}</span>
                <span>{{ formatClockTime(durationSec) }}</span>
              </div>
            </div>

            <!-- Buttons: Shuffle, Prev, Play/Pause, Next, Repeat -->
            <div class="flex items-center justify-center gap-4">
              <button
                @click="toggleShuffle"
                :class="[
                  'p-2 rounded-full transition-colors',
                  shuffle ? 'text-accent bg-accent/15' : 'text-text-secondary hover:text-ink hover:bg-elevated'
                ]"
                title="Aleatorio"
                aria-label="Aleatorio"
                data-testid="fullscreen-shuffle"
              >
                <span class="material-symbols-outlined text-xl">shuffle</span>
              </button>

              <button
                @click="previous"
                class="p-3 rounded-full bg-surface hover:bg-elevated text-ink transition-colors shadow"
                title="Pista anterior"
                aria-label="Pista anterior"
                data-testid="fullscreen-prev"
              >
                <span class="material-symbols-outlined text-2xl">skip_previous</span>
              </button>

              <button
                @click="toggle"
                class="p-4 rounded-full bg-accent hover:bg-accent/90 text-white transition-transform active:scale-95 shadow-xl flex items-center justify-center"
                :title="isPlaying ? 'Pausa' : 'Reproducir'"
                :aria-label="isPlaying ? 'Pausa' : 'Reproducir'"
                data-testid="fullscreen-toggle"
              >
                <span class="material-symbols-outlined text-3xl">
                  {{ isPlaying ? 'pause' : 'play_arrow' }}
                </span>
              </button>

              <button
                @click="next"
                class="p-3 rounded-full bg-surface hover:bg-elevated text-ink transition-colors shadow"
                title="Pista siguiente"
                aria-label="Pista siguiente"
                data-testid="fullscreen-next"
              >
                <span class="material-symbols-outlined text-2xl">skip_next</span>
              </button>

              <button
                @click="cycleRepeat"
                :class="[
                  'p-2 rounded-full transition-colors relative',
                  repeat !== 'off' ? 'text-accent bg-accent/15' : 'text-text-secondary hover:text-ink hover:bg-elevated'
                ]"
                :title="repeat === 'one' ? 'Repetir una' : repeat === 'all' ? 'Repetir todas' : 'No repetir'"
                aria-label="Modo repetición"
                data-testid="fullscreen-repeat"
              >
                <span class="material-symbols-outlined text-xl">
                  {{ repeat === 'one' ? 'repeat_one' : 'repeat' }}
                </span>
              </button>
            </div>

            <!-- Volume Slider -->
            <div class="flex items-center gap-3 max-w-xs mx-auto text-ink-2">
              <button
                @click="setMuted(!muted)"
                class="p-1 rounded hover:bg-elevated transition-colors text-text-secondary hover:text-ink"
                :title="muted ? 'Activar sonido' : 'Silenciar'"
                aria-label="Silenciar"
              >
                <span class="material-symbols-outlined text-lg">
                  {{ muted ? 'volume_off' : volume === 0 ? 'volume_mute' : volume < 0.5 ? 'volume_down' : 'volume_up' }}
                </span>
              </button>
              <input
                type="range"
                min="0"
                max="1"
                step="0.01"
                :value="muted ? 0 : volume"
                @input="onVolumeInput"
                class="flex-1 h-1.5 rounded-lg bg-surface appearance-none cursor-pointer accent-accent"
                aria-label="Volumen"
              />
            </div>
          </div>
        </section>

        <!-- Right Column: Interactive Content Panes (Tabs) -->
        <section class="lg:col-span-7 bg-surface/60 rounded-2xl border border-line p-6 flex flex-col min-h-0 overflow-hidden shadow-inner">
          <!-- 1. TAB: LYRICS (PL-04) -->
          <div
            v-if="activeTab === 'lyrics'"
            data-testid="panel-lyrics"
            class="flex-1 flex flex-col min-h-0"
          >
            <!-- Lyrics Header -->
            <div class="flex items-center justify-between pb-4 border-b border-line shrink-0">
              <div class="flex items-center gap-2">
                <span class="material-symbols-outlined text-accent text-lg">lyrics</span>
                <span class="text-sm font-semibold text-ink">Letras</span>
                <span
                  v-if="parsedLyrics.length > 0"
                  class="px-2 py-0.5 rounded-full text-[10px] bg-accent/20 text-accent font-medium"
                >
                  Sincronizadas (LRC)
                </span>
                <span
                  v-else-if="lyrics?.content"
                  class="px-2 py-0.5 rounded-full text-[10px] bg-elevated text-ink-2 font-medium"
                >
                  Texto Plano
                </span>
              </div>

              <div class="flex items-center gap-2 text-xs text-text-secondary">
                <span v-if="lyrics?.source" class="text-[11px] text-ink-3">
                  Fuente: {{ lyrics.source }}
                </span>
                <button
                  @click="refreshLyrics"
                  :disabled="lyricsLoading"
                  class="p-1 rounded hover:bg-elevated transition-colors text-text-secondary hover:text-ink"
                  title="Refrescar letras"
                >
                  <span class="material-symbols-outlined text-sm" :class="{ 'animate-spin': lyricsLoading }">refresh</span>
                </button>
              </div>
            </div>

            <!-- Lyrics Body -->
            <div
              ref="lyricsScrollContainer"
              class="flex-1 overflow-y-auto py-6 px-4 space-y-4 scroll-smooth min-h-0"
            >
              <!-- Synchronized LRC Lines -->
              <template v-if="parsedLyrics.length > 0">
                <div
                  v-for="(line, idx) in parsedLyrics"
                  :key="idx"
                  :ref="el => setLyricLineRef(el, idx)"
                  @click="seek(line.timeSec)"
                  :data-testid="`lyric-line-${idx}`"
                  :class="[
                    'cursor-pointer py-2 px-3 rounded-xl transition-all duration-300 text-left select-text',
                    activeLyricIndex === idx
                      ? 'text-accent font-bold text-xl sm:text-2xl bg-accent/10 scale-[1.02] shadow-sm'
                      : activeLyricIndex > idx
                        ? 'text-ink-3 text-base sm:text-lg hover:text-ink'
                        : 'text-text-secondary text-base sm:text-lg hover:text-ink'
                  ]"
                >
                  <p class="leading-relaxed">{{ line.text }}</p>
                </div>
              </template>

              <!-- Plain Text Lyrics -->
              <div
                v-else-if="lyrics?.content"
                class="whitespace-pre-line text-base sm:text-lg text-ink leading-relaxed font-sans px-2 select-text"
              >
                {{ lyrics.content }}
              </div>

              <!-- Empty State / No Lyrics Found -->
              <div
                v-else
                class="flex-1 flex flex-col items-center justify-center text-center py-16 space-y-3"
              >
                <span class="material-symbols-outlined text-4xl text-ink-3">music_off</span>
                <p class="text-sm font-medium text-ink">No hay letras disponibles para esta pista</p>
                <p class="text-xs text-text-secondary max-w-sm">
                  Las letras se descargan automáticamente o se pueden buscar en LRCLib y Genius.
                </p>
                <button
                  @click="refreshLyrics"
                  :disabled="lyricsLoading"
                  class="mt-2 px-4 py-1.5 rounded-lg bg-accent text-white text-xs font-medium hover:bg-accent/90 transition-colors flex items-center gap-1.5"
                >
                  <span class="material-symbols-outlined text-sm">search</span>
                  <span>{{ lyricsLoading ? 'Buscando letras...' : 'Buscar letras ahora' }}</span>
                </button>
              </div>
            </div>
          </div>

          <!-- 2. TAB: FULL METADATA (PL-04) -->
          <div
            v-else-if="activeTab === 'metadata'"
            data-testid="panel-metadata"
            class="flex-1 flex flex-col min-h-0"
          >
            <div class="flex items-center justify-between pb-4 border-b border-line shrink-0">
              <div class="flex items-center gap-2">
                <span class="material-symbols-outlined text-accent text-lg">info</span>
                <span class="text-sm font-semibold text-ink">Metadatos de la Pista</span>
              </div>
            </div>

            <div class="flex-1 overflow-y-auto py-4 space-y-6 min-h-0 text-xs">
              <!-- Grid: Información Básica -->
              <div class="grid grid-cols-2 sm:grid-cols-3 gap-4">
                <div class="p-3 rounded-xl bg-surface border border-line space-y-1">
                  <span class="text-text-secondary">Título</span>
                  <p class="font-medium text-ink truncate">{{ metadata?.title || current?.title || '—' }}</p>
                </div>
                <div class="p-3 rounded-xl bg-surface border border-line space-y-1">
                  <span class="text-text-secondary">Artista</span>
                  <p class="font-medium text-ink truncate">{{ metadata?.artist_name || current?.artist || '—' }}</p>
                </div>
                <div class="p-3 rounded-xl bg-surface border border-line space-y-1">
                  <span class="text-text-secondary">Álbum</span>
                  <p class="font-medium text-ink truncate">{{ metadata?.album_name || current?.album || '—' }}</p>
                </div>
                <div class="p-3 rounded-xl bg-surface border border-line space-y-1">
                  <span class="text-text-secondary">Pista / Disco</span>
                  <p class="font-medium text-ink">
                    {{ metadata?.track_number ? `#${metadata.track_number}` : '—' }}
                    {{ metadata?.disc_number ? `(Disco ${metadata.disc_number})` : '' }}
                  </p>
                </div>
                <div class="p-3 rounded-xl bg-surface border border-line space-y-1">
                  <span class="text-text-secondary">Año de Salida</span>
                  <p class="font-medium text-ink">{{ metadata?.release_year || '—' }}</p>
                </div>
                <div class="p-3 rounded-xl bg-surface border border-line space-y-1">
                  <span class="text-text-secondary">Género</span>
                  <p class="font-medium text-ink truncate">{{ metadata?.genre || '—' }}</p>
                </div>
              </div>

              <!-- Grid: Parámetros Musicales & Identificadores -->
              <div>
                <h3 class="text-xs font-semibold text-text-secondary uppercase tracking-wider mb-2">Identificadores y Audio</h3>
                <div class="grid grid-cols-2 sm:grid-cols-4 gap-3">
                  <div class="p-3 rounded-xl bg-surface border border-line space-y-1">
                    <span class="text-text-secondary">ISRC</span>
                    <p class="font-mono text-ink text-[11px] truncate select-all">{{ metadata?.isrc || '—' }}</p>
                  </div>
                  <div class="p-3 rounded-xl bg-surface border border-line space-y-1">
                    <span class="text-text-secondary">BPM</span>
                    <p class="font-mono text-ink">{{ metadata?.bpm || '—' }}</p>
                  </div>
                  <div class="p-3 rounded-xl bg-surface border border-line space-y-1">
                    <span class="text-text-secondary">Tonalidad</span>
                    <p class="font-mono text-ink">{{ metadata?.musical_key || '—' }}</p>
                  </div>
                  <div class="p-3 rounded-xl bg-surface border border-line space-y-1">
                    <span class="text-text-secondary">MusicBrainz ID</span>
                    <p class="font-mono text-ink text-[11px] truncate select-all">{{ metadata?.musicbrainz_id || '—' }}</p>
                  </div>
                </div>
              </div>

              <!-- Available Sources / Quality Details -->
              <div v-if="metadata?.sources && metadata.sources.length > 0">
                <h3 class="text-xs font-semibold text-text-secondary uppercase tracking-wider mb-2">Fuentes y Calidad</h3>
                <div class="space-y-2">
                  <div
                    v-for="source in metadata.sources"
                    :key="source.id"
                    class="p-3 rounded-xl bg-surface border border-line flex items-center justify-between"
                  >
                    <div class="flex items-center gap-2">
                      <span class="material-symbols-outlined text-base text-accent">album</span>
                      <span class="font-medium text-ink uppercase">{{ source.serviceName || 'Local' }}</span>
                      <span class="text-text-secondary">({{ source.format || 'Audio' }})</span>
                    </div>
                    <div class="flex items-center gap-3 font-mono text-[11px] text-ink-2">
                      <span v-if="source.bitDepth">{{ source.bitDepth }}-bit</span>
                      <span v-if="source.sampleRate">{{ (source.sampleRate / 1000).toFixed(1) }} kHz</span>
                      <span v-if="source.qualityScore" class="text-emerald-400">Score: {{ source.qualityScore }}</span>
                    </div>
                  </div>
                </div>
              </div>

              <!-- Local File Path -->
              <div v-if="metadata?.file_path" class="p-3 rounded-xl bg-surface border border-line space-y-1">
                <span class="text-text-secondary">Ubicación del Archivo Local</span>
                <p class="font-mono text-[11px] text-ink-2 break-all select-all">{{ metadata.file_path }}</p>
              </div>
            </div>
          </div>

          <!-- 3. TAB: ALBUM BOOKLET (PL-05) -->
          <div
            v-else-if="activeTab === 'booklet'"
            data-testid="panel-booklet"
            class="flex-1 flex flex-col min-h-0"
          >
            <div class="flex items-center justify-between pb-4 border-b border-line shrink-0">
              <div class="flex items-center gap-2">
                <span class="material-symbols-outlined text-accent text-lg">menu_book</span>
                <span class="text-sm font-semibold text-ink">Digital Booklet (.pdf)</span>
                <span
                  v-if="hasBooklet"
                  class="px-2 py-0.5 rounded-full text-[10px] bg-emerald-500/20 text-emerald-400 font-medium"
                >
                  Disponible
                </span>
              </div>

              <div v-if="hasBooklet" class="flex items-center gap-2">
                <button
                  @click="openBookletExternal"
                  class="px-3 py-1 rounded-lg bg-surface hover:bg-elevated text-ink text-xs transition-colors flex items-center gap-1 border border-line"
                  title="Abrir en visor del sistema"
                >
                  <span class="material-symbols-outlined text-sm">open_in_new</span>
                  <span>Abrir Externo</span>
                </button>
              </div>
            </div>

            <!-- Booklet Content -->
            <div class="flex-1 flex flex-col min-h-0 pt-4 overflow-hidden">
              <template v-if="hasBooklet && bookletUrl">
                <iframe
                  :src="bookletUrl"
                  data-testid="booklet-iframe"
                  class="w-full flex-1 rounded-xl border border-line bg-surface"
                  title="Visor de Digital Booklet"
                ></iframe>
              </template>

              <!-- Booklet Not Found State -->
              <div
                v-else
                class="flex-1 flex flex-col items-center justify-center text-center p-8 space-y-3"
              >
                <div class="w-16 h-16 rounded-2xl bg-surface border border-line flex items-center justify-center text-ink-3">
                  <span class="material-symbols-outlined text-3xl">menu_book</span>
                </div>
                <h3 class="text-base font-semibold text-ink">Sin Booklet Digital</h3>
                <p class="text-xs text-text-secondary max-w-md leading-relaxed">
                  Este álbum no cuenta con un booklet digital descargado (<code class="bg-surface px-1 py-0.5 rounded">booklet.pdf</code>).
                  Los digital booklets se obtienen automáticamente con álbumes soportados desde servicios como Qobuz cuando la opción
                  "Descargar digital booklets" está habilitada en Ajustes.
                </p>
                <div v-if="metadata?.file_path" class="mt-4 p-3 rounded-lg bg-surface border border-line text-left text-[11px] font-mono text-ink-3">
                  Directorio del álbum: {{ getAlbumDirectory(metadata.file_path) }}
                </div>
              </div>
            </div>
          </div>

          <!-- 4. TAB: QUEUE (PL-04) -->
          <div
            v-else-if="activeTab === 'queue'"
            data-testid="panel-queue"
            class="flex-1 flex flex-col min-h-0"
          >
            <div class="flex items-center justify-between pb-4 border-b border-line shrink-0">
              <div class="flex items-center gap-2">
                <span class="material-symbols-outlined text-accent text-lg">queue_music</span>
                <span class="text-sm font-semibold text-ink">Cola de Reproducción</span>
                <span class="px-2 py-0.5 rounded-full text-[10px] bg-elevated text-ink-2 font-mono">
                  {{ queue.length }} pendientes
                </span>
              </div>
            </div>

            <div class="flex-1 overflow-y-auto py-2 divide-y divide-line/40 min-h-0">
              <template v-if="queue.length > 0">
                <div
                  v-for="(entry, idx) in queue"
                  :key="entry.entryId"
                  class="py-2.5 px-3 flex items-center justify-between gap-3 hover:bg-elevated/60 rounded-xl transition-colors"
                >
                  <div class="flex items-center gap-3 min-w-0">
                    <span class="text-xs font-mono text-ink-3 w-5">{{ idx + 1 }}</span>
                    <TrackCover
                      :src="entry.track?.coverUrl"
                      :item-id="entry.track?.id"
                      :alt="entry.track?.title"
                      size-class="w-9 h-9"
                    />
                    <div class="min-w-0">
                      <p class="text-xs font-medium text-ink truncate">{{ entry.track?.title ?? `Pista #${entry.trackId}` }}</p>
                      <p class="text-[11px] text-text-secondary truncate">{{ entry.track?.artist ?? '—' }}</p>
                    </div>
                  </div>

                  <button
                    @click="removeEntry(entry.entryId)"
                    class="p-1 rounded hover:bg-elevated text-text-secondary hover:text-error transition-colors"
                    title="Quitar de la cola"
                  >
                    <span class="material-symbols-outlined text-sm">close</span>
                  </button>
                </div>
              </template>

              <div
                v-else
                class="flex-1 flex flex-col items-center justify-center text-center py-16 text-text-secondary space-y-2"
              >
                <span class="material-symbols-outlined text-3xl text-ink-3">queue_music</span>
                <p class="text-xs">No hay más pistas en la cola</p>
              </div>
            </div>
          </div>
        </section>
      </main>
    </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted, nextTick } from 'vue'
import { convertFileSrc } from '@tauri-apps/api/core'
import { usePlayer } from '../../composables/usePlayer'
import { getTrackMetadata, type TrackMetadataWithSources } from '../../api/library'
import { getLyrics } from '../../api/lyrics'
import type { Lyrics } from '../../api/types'
import { invokeCommand } from '../../api/tauri'
import { parseLrc, findActiveLyricIndex, type ParsedLyricLine } from '../../utils/lrcParser'
import TrackCover from '../TrackCover.vue'

const player = usePlayer()

const current = player.current ?? ref(null)
const isPlaying = player.isPlaying ?? ref(false)
const positionSec = player.positionSec ?? ref(0)
const durationSec = player.durationSec ?? ref(0)
const shuffle = player.shuffle ?? ref(false)
const repeat = player.repeat ?? ref('off')
const volume = player.volume ?? ref(1)
const muted = player.muted ?? ref(false)
const queue = player.queue ?? ref([])
const toggle = player.toggle ?? (() => {})
const next = player.next ?? (() => {})
const previous = player.previous ?? (() => {})
const seek = player.seek ?? (() => {})
const setVolume = player.setVolume ?? (() => {})
const setMuted = player.setMuted ?? (() => {})
const toggleShuffle = player.toggleShuffle ?? (() => {})
const cycleRepeat = player.cycleRepeat ?? (() => {})
const removeEntry = player.removeEntry ?? (() => {})
const isFullscreenOpen = player.isFullscreenOpen ?? ref(false)
const closeFullscreen = player.closeFullscreen ?? (() => {
  isFullscreenOpen.value = false
})

if (typeof window !== 'undefined') {
  window.addEventListener('syncify:open-fullscreen-player', () => {
    isFullscreenOpen.value = true
  })
}

// Tabs
const activeTab = ref<'lyrics' | 'metadata' | 'booklet' | 'queue'>('lyrics')

// Cover modes: animated vs hd
const coverViewMode = ref<'animated' | 'hd'>('animated')
const hasAnimatedCover = ref(false)
const animatedCoverUrl = ref<string | null>(null)
const hdCoverUrl = ref<string | null>(null)

// Booklet
const hasBooklet = ref(false)
const bookletUrl = ref<string | null>(null)
const bookletPath = ref<string | null>(null)

// Metadata & Lyrics
const metadata = ref<TrackMetadataWithSources | null>(null)
const lyrics = ref<Lyrics | null>(null)
const lyricsLoading = ref(false)
const isNativeFullscreen = ref(false)

// Lyrics element references for auto-scroll
const lyricsScrollContainer = ref<HTMLElement | null>(null)
const lyricLineElements: Record<number, HTMLElement> = {}

function setLyricLineRef(el: unknown, idx: number) {
  if (el && (el as HTMLElement).nodeType === 1) {
    lyricLineElements[idx] = el as HTMLElement
  }
}

// Parsed LRC lyrics
const parsedLyrics = computed<ParsedLyricLine[]>(() => {
  if (!lyrics.value?.content) return []
  return parseLrc(lyrics.value.content)
})

// Current active line index in LRC
const activeLyricIndex = computed<number>(() => {
  if (parsedLyrics.value.length === 0) return -1
  return findActiveLyricIndex(parsedLyrics.value, positionSec.value)
})

// Auto-scroll to active lyric line
watch(activeLyricIndex, (newIdx) => {
  if (newIdx < 0 || activeTab.value !== 'lyrics') return
  const el = lyricLineElements[newIdx]
  const container = lyricsScrollContainer.value
  if (el && container) {
    const top = el.offsetTop - container.offsetTop - (container.clientHeight / 2) + (el.clientHeight / 2)
    if (typeof container.scrollTo === 'function') {
      container.scrollTo({ top, behavior: 'smooth' })
    } else {
      container.scrollTop = top
    }
  }
})

// Audio Quality formatted badge
const qualityBadge = computed<string | null>(() => {
  if (!metadata.value?.sources || metadata.value.sources.length === 0) return null
  const src = metadata.value.sources[0]
  const parts: string[] = []
  if (src.format) parts.push(src.format)
  if (src.bitDepth && src.sampleRate) {
    parts.push(`${src.bitDepth}-bit / ${(src.sampleRate / 1000).toFixed(0)}kHz`)
  }
  return parts.length > 0 ? parts.join(' · ') : null
})

function formatClockTime(sec: number): string {
  if (!Number.isFinite(sec) || sec <= 0) return '0:00'
  const m = Math.floor(sec / 60)
  const s = Math.floor(sec % 60)
  return `${m}:${s.toString().padStart(2, '0')}`
}

function onVolumeInput(event: Event) {
  const target = event.target as HTMLInputElement
  setVolume(Number(target.value))
}

function onAnimatedCoverError() {
  hasAnimatedCover.value = false
  coverViewMode.value = 'hd'
}

function getAlbumDirectory(filePath: string): string {
  const sep = filePath.includes('\\') ? '\\' : '/'
  const lastIdx = filePath.lastIndexOf(sep)
  return lastIdx >= 0 ? filePath.substring(0, lastIdx) : ''
}

// Fetch metadata, lyrics, and check sidecars (booklet, animated cover)
async function loadCurrentTrackDetails() {
  if (!current.value) {
    metadata.value = null
    lyrics.value = null
    hasBooklet.value = false
    bookletUrl.value = null
    hasAnimatedCover.value = false
    animatedCoverUrl.value = null
    hdCoverUrl.value = null
    return
  }

  const trackId = current.value.id
  try {
    const [metaResult, lyrResult] = await Promise.all([
      getTrackMetadata(trackId).catch(() => null),
      getLyrics(trackId).catch(() => null),
    ])

    metadata.value = metaResult
    lyrics.value = lyrResult

    // Check files beside audio file
    if (metaResult?.file_path) {
      const sep = metaResult.file_path.includes('\\') ? '\\' : '/'
      const albumDir = getAlbumDirectory(metaResult.file_path)

      // 1. Digital Booklet (.pdf)
      const potentialBookletPath = `${albumDir}${sep}booklet.pdf`
      bookletPath.value = potentialBookletPath
      const potentialBookletUrl = convertFileSrc(potentialBookletPath)
      bookletUrl.value = potentialBookletUrl

      // Probe booklet presence
      try {
        const resp = await fetch(potentialBookletUrl, { method: 'HEAD' })
        hasBooklet.value = resp.ok
      } catch {
        // If fetch HEAD fails or in mock test env, assume booklet exists if test mock dictates
        hasBooklet.value = false
      }

      // 2. Animated Cover (.webp / .animated.webp)
      const potentialAnimPath = `${albumDir}${sep}cover.webp`
      const potentialAnimUrl = convertFileSrc(potentialAnimPath)
      animatedCoverUrl.value = potentialAnimUrl

      // 3. HD Static Cover
      hdCoverUrl.value = metaResult.cover_art_url || convertFileSrc(`${albumDir}${sep}cover.jpg`)

      // Test animated cover via Image probe
      if (typeof Image !== 'undefined') {
        const testImg = new Image()
        testImg.onload = () => {
          hasAnimatedCover.value = true
        }
        testImg.onerror = () => {
          hasAnimatedCover.value = false
        }
        testImg.src = potentialAnimUrl
      }
    } else {
      hdCoverUrl.value = metaResult?.cover_art_url || current.value.coverUrl || null
      hasAnimatedCover.value = false
      hasBooklet.value = false
    }
  } catch (err) {
    console.warn('[FullscreenPlayer] Error loading track details:', err)
  }
}

async function refreshLyrics() {
  if (!current.value) return
  lyricsLoading.value = true
  try {
    const trackId = current.value.id
    // Try resolve_track_lyrics or getLyrics
    const lyr = await getLyrics(trackId)
    if (lyr) {
      lyrics.value = lyr
    } else {
      // Try resolving
      const resolved = await invokeCommand<any>('resolve_track_lyrics', {
        artist: current.value.artist,
        title: current.value.title,
        album: current.value.album || null,
        durationSec: durationSec.value || null,
        filePath: metadata.value?.file_path || null,
        trackId,
      }).catch(() => null)
      if (resolved?.lyrics) {
        lyrics.value = resolved.lyrics
      }
    }
  } finally {
    lyricsLoading.value = false
  }
}

function openBookletExternal() {
  if (bookletUrl.value && typeof window !== 'undefined') {
    window.open(bookletUrl.value, '_blank')
  }
}

function toggleNativeFullscreen() {
  if (typeof document === 'undefined') return
  if (!document.fullscreenElement) {
    document.documentElement.requestFullscreen?.().then(() => {
      isNativeFullscreen.value = true
    }).catch(() => {})
  } else {
    document.exitFullscreen?.().then(() => {
      isNativeFullscreen.value = false
    }).catch(() => {})
  }
}

function handleKeydown(e: KeyboardEvent) {
  if (!isFullscreenOpen.value) return
  if (e.key === 'Escape') {
    closeFullscreen()
  }
}

// Watch track change or opening of player
watch(() => current.value?.id, () => {
  if (isFullscreenOpen.value) {
    void loadCurrentTrackDetails()
  }
})

watch(isFullscreenOpen, (isOpen) => {
  if (isOpen) {
    void loadCurrentTrackDetails()
    nextTick(() => {
      window.addEventListener('keydown', handleKeydown)
    })
  } else {
    window.removeEventListener('keydown', handleKeydown)
  }
})

onMounted(() => {
  if (typeof window !== 'undefined') {
    window.addEventListener('keydown', handleKeydown)
  }
  if (isFullscreenOpen.value) {
    void loadCurrentTrackDetails()
  }
})

onUnmounted(() => {
  if (typeof window !== 'undefined') {
    window.removeEventListener('keydown', handleKeydown)
  }
})
</script>

<style scoped>
.fullscreen-fade-enter-active,
.fullscreen-fade-leave-active {
  transition: opacity 0.3s ease, transform 0.3s ease;
}

.fullscreen-fade-enter-from,
.fullscreen-fade-leave-to {
  opacity: 0;
  transform: scale(0.98);
}
</style>
