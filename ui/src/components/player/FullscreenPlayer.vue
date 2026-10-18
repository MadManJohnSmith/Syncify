<template>
  <!-- UI-CORR-1 (10-oct-2026): escape the stacking context of the
       `StatusBar` footer (z-[50]/z-[160]) and of `main` with overflow-hidden. A
       z-[200] child inside that footer could never cover the rest of the app;
       Teleport makes it a direct child of body and keeps the Close/Esc buttons
       and the usePlayer state without duplicating players. -->
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
            <h1 class="text-sm font-semibold tracking-wider uppercase text-ink">Full Player</h1>
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
            <span>Lyrics</span>
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
            <span>Metadata</span>
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
              title="Booklet available"
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
            <span>Queue ({{ queue.length }})</span>
          </button>
        </nav>

        <!-- Right Window Actions -->
        <div class="flex items-center gap-2">
          <button
            @click="toggleNativeFullscreen"
            class="p-2 rounded-lg bg-surface hover:bg-elevated text-text-secondary hover:text-ink transition-colors"
            title="Window fullscreen"
            aria-label="Window fullscreen"
          >
            <span class="material-symbols-outlined text-lg">
              {{ isNativeFullscreen ? 'fullscreen_exit' : 'fullscreen' }}
            </span>
          </button>

          <button
            @click="closeFullscreen"
            data-testid="close-fullscreen-player"
            class="p-2 rounded-lg bg-surface hover:bg-elevated text-text-secondary hover:text-ink transition-colors"
            title="Close player (Esc)"
            aria-label="Close player"
          >
            <span class="material-symbols-outlined text-lg">close</span>
          </button>
        </div>
      </header>

      <!-- Main Body Container -->
      <main class="relative z-10 flex-1 grid grid-cols-1 lg:grid-cols-12 gap-8 p-6 lg:p-10 min-h-0 overflow-y-auto lg:overflow-hidden">
        <!-- Left Column: Artwork & Playback Transport -->
        <section class="lg:col-span-5 flex flex-col justify-between max-w-2xl mx-auto w-full min-h-0 space-y-6">
          <!-- Artwork / Animated Cover Section (PL-04) -->
          <div class="flex flex-col items-center">
            <!-- Cover View Switcher (Animated vs Static HD) -->
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
                <span>Animated</span>
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
                <span>Static HD</span>
              </button>
            </div>

            <!-- Cover Box -->
            <!-- UI-CORR-2: la carátula crece ~50% en cada breakpoint y queda
                 completamente libre: la etiqueta de calidad vive FUERA de este
                 contenedor (debajo), nunca encima del arte. -->
            <div
              data-testid="fullscreen-cover-container"
              class="relative w-full aspect-square max-w-[24rem] sm:max-w-[30rem] md:max-w-[36rem] rounded-2xl overflow-hidden shadow-2xl border border-line/60 bg-surface flex items-center justify-center group"
            >
              <!-- Animated Cover (WebP) -->
              <img
                v-if="hasAnimatedCover && coverViewMode === 'animated' && !animatedCoverIsVideo"
                :src="animatedCoverUrl!"
                alt="Animated cover"
                data-testid="animated-cover-img"
                class="w-full h-full object-cover transition-transform duration-500 group-hover:scale-105"
                @error="onAnimatedCoverError()"
              />

              <!-- Animated Cover (MP4 sidecar: `animated_cover.mp4`) -->
              <video
                v-else-if="hasAnimatedCover && coverViewMode === 'animated'"
                :src="animatedCoverUrl!"
                data-testid="animated-cover-video"
                class="w-full h-full object-cover transition-transform duration-500 group-hover:scale-105"
                autoplay
                loop
                muted
                playsinline
                preload="metadata"
                @error="onAnimatedCoverError"
              ></video>

              <!-- Static HD Cover / Fallback (TrackCover) -->
              <template v-else>
                <img
                  v-if="hdCoverUrl"
                  :src="hdCoverUrl"
                  :alt="current?.title ?? 'HD cover'"
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
            </div>

            <!-- Audio Quality Badge: fuera de la carátula (nada la tapa) -->
            <div
              v-if="qualityBadge"
              class="mt-3 flex flex-wrap items-center justify-center gap-2"
            >
              <span
                class="bg-black/75 backdrop-blur-md px-2.5 py-1 rounded-md text-[11px] font-mono font-medium text-emerald-400 border border-emerald-500/30 shadow-lg flex items-center gap-1"
                data-testid="audio-quality-badge"
                :title="qualityBadgeTitle"
              >
                <span class="material-symbols-outlined text-xs">graphic_eq</span>
                <span>{{ qualityBadge }}</span>
              </span>
              <span
                v-if="qualityBadgeSource"
                class="px-2 py-1 rounded-md text-[10px] font-medium bg-surface border border-line text-text-secondary"
                data-testid="audio-quality-source"
              >
                {{ qualityBadgeSource }}
              </span>
            </div>
          </div>

          <!-- Track Information -->
          <div class="text-center space-y-1">
            <h2
              data-testid="fullscreen-title"
              class="text-xl sm:text-2xl font-bold text-ink truncate px-2"
              :title="current?.title"
            >
              {{ current?.title ?? 'Nothing playing' }}
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
                  aria-label="Playback position"
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
                title="Shuffle"
                aria-label="Shuffle"
                data-testid="fullscreen-shuffle"
              >
                <span class="material-symbols-outlined text-xl">shuffle</span>
              </button>

              <button
                @click="previous"
                class="p-3 rounded-full bg-surface hover:bg-elevated text-ink transition-colors shadow"
                title="Previous track"
                aria-label="Previous track"
                data-testid="fullscreen-prev"
              >
                <span class="material-symbols-outlined text-2xl">skip_previous</span>
              </button>

              <button
                @click="toggle"
                class="p-4 rounded-full bg-accent hover:bg-accent/90 text-white transition-transform active:scale-95 shadow-xl flex items-center justify-center"
                :title="isPlaying ? 'Pause' : 'Play'"
                :aria-label="isPlaying ? 'Pause' : 'Play'"
                data-testid="fullscreen-toggle"
              >
                <span class="material-symbols-outlined text-3xl">
                  {{ isPlaying ? 'pause' : 'play_arrow' }}
                </span>
              </button>

              <button
                @click="next"
                class="p-3 rounded-full bg-surface hover:bg-elevated text-ink transition-colors shadow"
                title="Next track"
                aria-label="Next track"
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
                :title="repeat === 'one' ? 'Repeat one' : repeat === 'all' ? 'Repeat all' : 'No repeat'"
                aria-label="Repeat mode"
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
                :title="muted ? 'Unmute' : 'Mute'"
                aria-label="Mute"
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
                aria-label="Volume"
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
                <span class="text-sm font-semibold text-ink">Lyrics</span>
                <span
                  v-if="parsedLyrics.length > 0"
                  class="px-2 py-0.5 rounded-full text-[10px] bg-accent/20 text-accent font-medium"
                >
                  Synchronized (LRC)
                </span>
                <span
                  v-else-if="lyrics?.content"
                  class="px-2 py-0.5 rounded-full text-[10px] bg-elevated text-ink-2 font-medium"
                >
                  Plain Text
                </span>
              </div>

              <div class="flex items-center gap-2 text-xs text-text-secondary">
                <span v-if="lyrics?.source" class="text-[11px] text-ink-3">
                  Source: {{ lyrics.source }}
                </span>
                <button
                  @click="refreshLyrics"
                  :disabled="lyricsLoading"
                  class="p-1 rounded hover:bg-elevated transition-colors text-text-secondary hover:text-ink"
                  title="Refresh lyrics"
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
                <p class="text-sm font-medium text-ink">No lyrics available for this track</p>
                <p class="text-xs text-text-secondary max-w-sm">
                  Lyrics are downloaded automatically or can be searched on LRCLib and Genius.
                </p>
                <button
                  @click="refreshLyrics"
                  :disabled="lyricsLoading"
                  class="mt-2 px-4 py-1.5 rounded-lg bg-accent text-white text-xs font-medium hover:bg-accent/90 transition-colors flex items-center gap-1.5"
                >
                  <span class="material-symbols-outlined text-sm">search</span>
                  <span>{{ lyricsLoading ? 'Searching for lyrics...' : 'Search for lyrics now' }}</span>
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
                <span class="text-sm font-semibold text-ink">Track Metadata</span>
              </div>
            </div>

            <div class="flex-1 overflow-y-auto py-4 space-y-6 min-h-0 text-xs">
              <!-- Grid: Basic Information -->
              <div class="grid grid-cols-2 sm:grid-cols-3 gap-4">
                <div class="p-3 rounded-xl bg-surface border border-line space-y-1">
                  <span class="text-text-secondary">Title</span>
                  <p class="font-medium text-ink truncate">{{ metadata?.title || current?.title || '—' }}</p>
                </div>
                <div class="p-3 rounded-xl bg-surface border border-line space-y-1">
                  <span class="text-text-secondary">Artist</span>
                  <p class="font-medium text-ink truncate">{{ metadata?.artist_name || current?.artist || '—' }}</p>
                </div>
                <div class="p-3 rounded-xl bg-surface border border-line space-y-1">
                  <span class="text-text-secondary">Album Artist</span>
                  <p class="font-medium text-ink truncate">{{ metadata?.album_artist || '—' }}</p>
                </div>
                <div class="p-3 rounded-xl bg-surface border border-line space-y-1">
                  <span class="text-text-secondary">Album</span>
                  <p class="font-medium text-ink truncate">{{ metadata?.album_name || current?.album || '—' }}</p>
                </div>
                <div class="p-3 rounded-xl bg-surface border border-line space-y-1">
                  <span class="text-text-secondary">Track / Disc</span>
                  <p class="font-medium text-ink">
                    {{ metadata?.track_number ? `#${metadata.track_number}` : '—' }}
                    <span v-if="metadata?.total_tracks" class="text-text-secondary">/ {{ metadata.total_tracks }}</span>
                    {{ metadata?.disc_number ? `(Disc ${metadata.disc_number}${metadata?.total_discs ? `/${metadata.total_discs}` : ''})` : '' }}
                  </p>
                </div>
                <div class="p-3 rounded-xl bg-surface border border-line space-y-1">
                  <span class="text-text-secondary">Release Date</span>
                  <p class="font-medium text-ink">{{ metadata?.release_date || metadata?.release_year || '—' }}</p>
                </div>
                <div class="p-3 rounded-xl bg-surface border border-line space-y-1">
                  <span class="text-text-secondary">Genre</span>
                  <p class="font-medium text-ink truncate">{{ metadata?.genre || '—' }}</p>
                </div>
                <div class="p-3 rounded-xl bg-surface border border-line space-y-1">
                  <span class="text-text-secondary">Label</span>
                  <p class="font-medium text-ink truncate">{{ metadata?.label || '—' }}</p>
                </div>
                <div class="p-3 rounded-xl bg-surface border border-line space-y-1">
                  <span class="text-text-secondary">Duration</span>
                  <p class="font-mono text-ink">{{ formatDurationLabel(metadata?.duration_ms ?? durationSec * 1000) }}</p>
                </div>
                <div class="p-3 rounded-xl bg-surface border border-line space-y-1">
                  <span class="text-text-secondary">Explicit</span>
                  <p class="font-medium text-ink">{{ metadata?.explicit === null || metadata?.explicit === undefined ? '—' : (metadata.explicit ? 'Yes' : 'No') }}</p>
                </div>
                <div class="p-3 rounded-xl bg-surface border border-line space-y-1">
                  <span class="text-text-secondary">Added from</span>
                  <p class="font-medium text-ink truncate">{{ metadata?.imported_from || metadata?.downloaded_from || '—' }}</p>
                </div>
                <div class="p-3 rounded-xl bg-surface border border-line space-y-1">
                  <span class="text-text-secondary">Downloaded from</span>
                  <p class="font-medium text-ink truncate">{{ metadata?.downloaded_from || '—' }}</p>
                </div>
              </div>

              <!-- Grid: Musical Parameters & Identifiers -->
              <div>
                <h3 class="text-xs font-semibold text-text-secondary uppercase tracking-wider mb-2">Identifiers and Audio</h3>
                <div class="grid grid-cols-2 sm:grid-cols-4 gap-3">
                  <div class="p-3 rounded-xl bg-surface border border-line space-y-1">
                    <span class="text-text-secondary">ISRC</span>
                    <p class="font-mono text-ink text-[11px] truncate select-all">{{ metadata?.isrc || '—' }}</p>
                  </div>
                  <div class="p-3 rounded-xl bg-surface border border-line space-y-1">
                    <span class="text-text-secondary">BPM</span>
                    <p class="font-mono text-ink">{{ metadata?.bpm ?? '—' }}</p>
                    <span v-if="metadata?.tempo_source" class="text-[10px] text-ink-3 truncate">
                      {{ metadata.tempo_source }}<template v-if="metadata?.tempo_confidence"> · {{ Math.round(metadata.tempo_confidence * 100) }}%</template>
                    </span>
                  </div>
                  <div class="p-3 rounded-xl bg-surface border border-line space-y-1">
                    <span class="text-text-secondary">Key</span>
                    <p class="font-mono text-ink">{{ metadata?.musical_key || '—' }}</p>
                  </div>
                  <div class="p-3 rounded-xl bg-surface border border-line space-y-1">
                    <span class="text-text-secondary">UPC / EAN</span>
                    <p class="font-mono text-ink text-[11px] truncate select-all">{{ metadata?.upc || '—' }}</p>
                  </div>
                  <div class="p-3 rounded-xl bg-surface border border-line space-y-1">
                    <span class="text-text-secondary">Track MBID</span>
                    <p class="font-mono text-ink text-[11px] truncate select-all">{{ metadata?.musicbrainz_id || '—' }}</p>
                  </div>
                  <div class="p-3 rounded-xl bg-surface border border-line space-y-1">
                    <span class="text-text-secondary">Album MBID</span>
                    <p class="font-mono text-ink text-[11px] truncate select-all">{{ metadata?.album_musicbrainz_id || '—' }}</p>
                  </div>
                  <div class="p-3 rounded-xl bg-surface border border-line space-y-1">
                    <span class="text-text-secondary">Release MBID</span>
                    <p class="font-mono text-ink text-[11px] truncate select-all">{{ metadata?.musicbrainz_release_id || '—' }}</p>
                  </div>
                  <div class="p-3 rounded-xl bg-surface border border-line space-y-1">
                    <span class="text-text-secondary">File disambiguator</span>
                    <p class="font-mono text-ink text-[11px] truncate">{{ metadata?.file_disambiguator || '—' }}</p>
                  </div>
                </div>
              </div>

              <!-- Local file technical data (from the downloaded file row) -->
              <div v-if="metadata?.file_path || metadata?.file_format">
                <h3 class="text-xs font-semibold text-text-secondary uppercase tracking-wider mb-2">Local File</h3>
                <div class="grid grid-cols-2 sm:grid-cols-4 gap-3">
                  <div class="p-3 rounded-xl bg-surface border border-line space-y-1">
                    <span class="text-text-secondary">Format</span>
                    <p class="font-mono text-ink">{{ metadata?.file_format || '—' }}</p>
                  </div>
                  <div class="p-3 rounded-xl bg-surface border border-line space-y-1">
                    <span class="text-text-secondary">Bit depth</span>
                    <p class="font-mono text-ink">{{ metadata?.bit_depth ? `${metadata.bit_depth}-bit` : '—' }}</p>
                  </div>
                  <div class="p-3 rounded-xl bg-surface border border-line space-y-1">
                    <span class="text-text-secondary">Sample rate</span>
                    <p class="font-mono text-ink">{{ metadata?.sample_rate ? `${(metadata.sample_rate / 1000).toFixed(1)} kHz` : '—' }}</p>
                  </div>
                  <div class="p-3 rounded-xl bg-surface border border-line space-y-1">
                    <span class="text-text-secondary">File size</span>
                    <p class="font-mono text-ink">{{ formatFileSize(metadata?.file_size_bytes) }}</p>
                  </div>
                  <div class="p-3 rounded-xl bg-surface border border-line space-y-1">
                    <span class="text-text-secondary">Downloaded at</span>
                    <p class="font-mono text-ink">{{ formatTimestamp(metadata?.downloaded_at) }}</p>
                  </div>
                  <div class="p-3 rounded-xl bg-surface border border-line space-y-1">
                    <span class="text-text-secondary">Cover type</span>
                    <p class="font-mono text-ink">{{ coverKindLabel }}</p>
                  </div>
                </div>
              </div>

              <!-- Raw container tags (read straight from the audio file) -->
              <div v-if="fileTagEntries.length > 0">
                <h3 class="text-xs font-semibold text-text-secondary uppercase tracking-wider mb-2">
                  Tags in file ({{ fileTagEntries.length }})
                </h3>
                <div class="p-3 rounded-xl bg-surface border border-line space-y-1.5">
                  <div
                    v-for="tag in fileTagEntries"
                    :key="tag.key"
                    class="flex items-start justify-between gap-4 border-b border-line/40 last:border-0 pb-1.5 last:pb-0"
                  >
                    <span class="font-mono text-[11px] text-text-secondary shrink-0 pt-0.5">{{ tag.key }}</span>
                    <span class="font-mono text-[11px] text-ink text-right break-all select-all">{{ tag.value }}</span>
                  </div>
                </div>
              </div>

              <!-- Available Sources / Quality Details -->
              <div v-if="metadata?.sources && metadata.sources.length > 0">
                <h3 class="text-xs font-semibold text-text-secondary uppercase tracking-wider mb-2">Sources and Quality</h3>
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
                <span class="text-text-secondary">Local File Location</span>
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
                  Available
                </span>
              </div>

              <div v-if="hasBooklet" class="flex items-center gap-2">
                <button
                  @click="openBookletExternal"
                  data-testid="booklet-open-external"
                  class="px-3 py-1 rounded-lg bg-surface hover:bg-elevated text-ink text-xs transition-colors flex items-center gap-1 border border-line"
                  title="Open in system viewer"
                >
                  <span class="material-symbols-outlined text-sm">open_in_new</span>
                  <span>Open Externally</span>
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
                  title="Digital Booklet viewer"
                  @load="bookletLoadFailed = false"
                  @error="bookletLoadFailed = true"
                ></iframe>
                <!-- Fallback honesto: si el visor embebido no pinta el PDF
                     (WebKit sin PDF.js, CSP estricta…), se dice y se ofrece
                     abrirlo fuera en vez de dejar un iframe en blanco. -->
                <div
                  v-if="bookletLoadFailed"
                  data-testid="booklet-viewer-fallback"
                  class="mt-2 flex items-center justify-between gap-3 rounded-lg border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-[11px] text-amber-300"
                >
                  <span>
                    The embedded viewer could not render this PDF. Open it with the system viewer.
                  </span>
                  <button
                    @click="openBookletExternal"
                    class="px-2.5 py-1 rounded-md bg-surface hover:bg-elevated text-ink transition-colors border border-line shrink-0"
                  >
                    Open Externally
                  </button>
                </div>
              </template>

              <!-- Booklet Not Found State -->
              <div
                v-else
                class="flex-1 flex flex-col items-center justify-center text-center p-8 space-y-3"
              >
                <div class="w-16 h-16 rounded-2xl bg-surface border border-line flex items-center justify-center text-ink-3">
                  <span class="material-symbols-outlined text-3xl">menu_book</span>
                </div>
                <h3 class="text-base font-semibold text-ink">No Digital Booklet</h3>
                <p class="text-xs text-text-secondary max-w-md leading-relaxed">
                  This album has no downloaded digital booklet (<code class="bg-surface px-1 py-0.5 rounded">booklet.pdf</code>).
                  Digital booklets are fetched automatically for supported albums from services like Qobuz when the
                  "Download digital booklets" option is enabled in Settings.
                </p>
                <div v-if="metadata?.file_path" class="mt-4 p-3 rounded-lg bg-surface border border-line text-left text-[11px] font-mono text-ink-3">
                  Album directory: {{ getAlbumDirectory(metadata.file_path) }}
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
                <span class="text-sm font-semibold text-ink">Playback Queue</span>
                <span class="px-2 py-0.5 rounded-full text-[10px] bg-elevated text-ink-2 font-mono">
                  {{ queue.length }} pending
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
                      <p class="text-xs font-medium text-ink truncate">{{ entry.track?.title ?? `Track #${entry.trackId}` }}</p>
                      <p class="text-[11px] text-text-secondary truncate">{{ entry.track?.artist ?? '—' }}</p>
                    </div>
                  </div>

                  <button
                    @click="removeEntry(entry.entryId)"
                    class="p-1 rounded hover:bg-elevated text-text-secondary hover:text-error transition-colors"
                    title="Remove from queue"
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
                <p class="text-xs">No more tracks in the queue</p>
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
import { readTrackTags } from '../../api/metadata'
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
const animatedCoverIsVideo = ref(false)
const hdCoverUrl = ref<string | null>(null)

// Booklet
const hasBooklet = ref(false)
const bookletUrl = ref<string | null>(null)
const bookletPath = ref<string | null>(null)
const bookletLoadFailed = ref(false)

// Metadata & Lyrics
const metadata = ref<TrackMetadataWithSources | null>(null)
const fileTags = ref<Record<string, string[]>>({})
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

// Audio Quality formatted badge.
// Streaming wins: the honest label reflects what the engine really decodes
// (the backend verifies the stream header and rejects 30 s previews), not what
// the library row promised. Falls back to the best available source row.
const streamQuality = player.streamQuality ?? ref(null)

const qualityBadge = computed<string | null>(() => {
  const stream = streamQuality.value
  if (stream?.qualityLabel) return stream.qualityLabel

  const src = metadata.value?.sources?.[0]
  if (src) {
    const parts: string[] = []
    if (src.format) parts.push(src.format)
    if (src.bitDepth && src.sampleRate) {
      parts.push(`${src.bitDepth}-bit / ${(src.sampleRate / 1000).toFixed(0)}kHz`)
    }
    if (parts.length > 0) return parts.join(' · ')
  }

  // No source rows (e.g. a bare streamed track): use the local file row.
  if (metadata.value?.file_format) {
    const bits: string[] = [metadata.value.file_format]
    if (metadata.value.bit_depth && metadata.value.sample_rate) {
      bits.push(
        `${metadata.value.bit_depth}-bit / ${(metadata.value.sample_rate / 1000).toFixed(0)}kHz`,
      )
    }
    return bits.join(' · ')
  }
  return null
})

const qualityBadgeTitle = computed(() =>
  streamQuality.value
    ? `Streaming from ${streamQuality.value.serviceName} · obtained quality (verified stream)`
    : 'Best available source quality for this track',
)

const qualityBadgeSource = computed<string | null>(() => {
  const stream = streamQuality.value
  if (stream) {
    const tier = stream.qualityTier
    const tierLabel = tier === 'hires' ? 'Hi-Res' : tier === 'lossless' ? 'Lossless' : 'Lossy'
    return `${stream.serviceName.toUpperCase()} · ${tierLabel}`
  }
  const src = metadata.value?.sources?.[0]
  if (src?.serviceName) return src.serviceName.toUpperCase()
  return metadata.value?.downloaded_from?.toUpperCase() ?? null
})

const coverKindLabel = computed<string>(() => {
  if (animatedCoverUrl.value && hasAnimatedCover.value) {
    return animatedCoverIsVideo.value ? 'Animated (MP4)' : 'Animated (WebP)'
  }
  if (metadata.value?.hd_cover_path) return 'Static (sidecar)'
  if (metadata.value?.file_path) return 'Embedded (audio file)'
  return metadata.value?.cover_art_url ? 'Remote artwork' : '—'
})

const fileTagEntries = computed<Array<{ key: string; value: string }>>(() => {
  const entries: Array<{ key: string; value: string }> = []
  for (const key of Object.keys(fileTags.value).sort()) {
    const values = fileTags.value[key]
    if (!Array.isArray(values) || values.length === 0) continue
    entries.push({ key, value: values.join(' / ') })
  }
  return entries
})

function formatDurationLabel(ms: number | null | undefined): string {
  if (!ms || !Number.isFinite(ms) || ms <= 0) return '—'
  const totalSec = Math.round(ms / 1000)
  const h = Math.floor(totalSec / 3600)
  const m = Math.floor((totalSec % 3600) / 60)
  const s = totalSec % 60
  return h > 0
    ? `${h}:${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`
    : `${m}:${String(s).padStart(2, '0')}`
}

function formatFileSize(bytes: number | null | undefined): string {
  if (!bytes || !Number.isFinite(bytes) || bytes <= 0) return '—'
  const mb = bytes / (1024 * 1024)
  return mb >= 1 ? `${mb.toFixed(1)} MB` : `${Math.round(bytes / 1024)} KB`
}

function formatTimestamp(value: string | null | undefined): string {
  if (!value) return '—'
  // SQLite CURRENT_TIMESTAMP is `YYYY-MM-DD HH:MM:SS` (UTC); Date parses it.
  const date = new Date(value.includes('T') ? value : value.replace(' ', 'T') + 'Z')
  if (Number.isNaN(date.getTime())) return value
  return date.toLocaleString()
}

function isVideoPath(path: string | null | undefined): boolean {
  return !!path && /\.(mp4|m4v|webm|mov)$/i.test(path)
}

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

/**
 * Turns a backend filesystem path into a URL the webview actually PAINTS.
 *
 * Why not `convertFileSrc` directly: on Linux/WebKitGTK a custom scheme
 * (`asset://…`) answers `fetch()` with 200 and the right `Content-Type`, but the
 * same URL used as a subresource (`<img>`, `<iframe>`, `<video>`) fails — the
 * webview refuses to display custom-scheme resources loaded from an http(s)
 * page. Fetching the bytes and wrapping them in a `blob:` URL is portable
 * (WebKitGTK, WebView2, WKWebView) and keeps animation/PDF/video intact. The
 * `asset://` URL is still returned as a fallback when the fetch fails.
 *
 * Object URLs are cached per path and revoked when the track changes to keep
 * memory bounded (a booklet can be tens of MB).
 */
const sidecarObjectUrls = new Map<string, string>()

function revokeSidecarUrls(): void {
  for (const url of sidecarObjectUrls.values()) {
    try {
      URL.revokeObjectURL(url)
    } catch {
      /* not a blob URL: nothing to revoke */
    }
  }
  sidecarObjectUrls.clear()
}

async function displayableUrl(path: string | null | undefined): Promise<string | null> {
  if (!path) return null
  const cached = sidecarObjectUrls.get(path)
  if (cached) return cached

  const assetUrl = convertFileSrc(path)
  try {
    const response = await fetch(assetUrl)
    if (!response.ok) return assetUrl
    const blob = await response.blob()
    const objectUrl = URL.createObjectURL(blob)
    sidecarObjectUrls.set(path, objectUrl)
    return objectUrl
  } catch {
    // Environments without the asset protocol (unit tests, plain browser):
    // keep the original URL so callers can still decide what to do.
    return assetUrl
  }
}

// Fetch metadata, lyrics, sidecar paths and raw file tags for the current track.
//
// Sidecar discovery (booklet PDF, animated WebP/MP4, HD cover) now happens in
// the backend (`fetch_track_metadata`) against the real filesystem: the webview
// only receives absolute paths that exist. That removes the old `fetch(HEAD)`
// probe, which could never work because the asset protocol only answers GET.
// The legacy probes below stay as a best-effort fallback for environments that
// do not provide the new fields (older mocks, remote artwork only).
async function loadCurrentTrackDetails() {
  if (!current.value) {
    metadata.value = null
    lyrics.value = null
    fileTags.value = {}
    hasBooklet.value = false
    bookletUrl.value = null
    bookletPath.value = null
    bookletLoadFailed.value = false
    hasAnimatedCover.value = false
    animatedCoverUrl.value = null
    animatedCoverIsVideo.value = false
    hdCoverUrl.value = null
    return
  }

  const trackId = current.value.id
  revokeSidecarUrls()
  try {
    const [metaResult, lyrResult] = await Promise.all([
      getTrackMetadata(trackId).catch(() => null),
      getLyrics(trackId).catch(() => null),
    ])

    // A slower response from a previous track must not overwrite this one.
    if (current.value?.id !== trackId) return

    metadata.value = metaResult
    lyrics.value = lyrResult
    bookletLoadFailed.value = false

    // 1. Digital booklet: authoritative path from the backend.
    if (metaResult?.booklet_path) {
      bookletPath.value = metaResult.booklet_path
      bookletUrl.value = await displayableUrl(metaResult.booklet_path)
      hasBooklet.value = Boolean(bookletUrl.value)
    } else if (metaResult?.file_path) {
      const sep = metaResult.file_path.includes('\\') ? '\\' : '/'
      const albumDir = getAlbumDirectory(metaResult.file_path)
      bookletPath.value = `${albumDir}${sep}booklet.pdf`
      bookletUrl.value = null
      hasBooklet.value = false
    } else {
      bookletPath.value = null
      bookletUrl.value = null
      hasBooklet.value = false
    }

    // 2. Animated cover: WebP (`cover.animated.webp`, `animated.webp`, or an
    //    animated `cover.webp`) or the `animated_cover.mp4` sidecar.
    if (metaResult?.animated_cover_path) {
      animatedCoverUrl.value = await displayableUrl(metaResult.animated_cover_path)
      animatedCoverIsVideo.value = isVideoPath(metaResult.animated_cover_path)
      hasAnimatedCover.value = Boolean(animatedCoverUrl.value)
      coverViewMode.value = 'animated'
    } else if (metaResult?.file_path) {
      const sep = metaResult.file_path.includes('\\') ? '\\' : '/'
      const albumDir = getAlbumDirectory(metaResult.file_path)
      const potentialAnimPath = `${albumDir}${sep}cover.webp`
      const potentialAnimUrl = convertFileSrc(potentialAnimPath)
      animatedCoverUrl.value = potentialAnimUrl
      animatedCoverIsVideo.value = false
      hasAnimatedCover.value = false

      // Fallback probe for environments without backend sidecar discovery.
      if (typeof Image !== 'undefined') {
        const testImg = new Image()
        testImg.onload = () => {
          if (current.value?.id === trackId) {
            hasAnimatedCover.value = true
            coverViewMode.value = 'animated'
          }
        }
        testImg.onerror = () => {
          if (current.value?.id === trackId) hasAnimatedCover.value = false
        }
        testImg.src = potentialAnimUrl
      }
    } else {
      animatedCoverUrl.value = null
      animatedCoverIsVideo.value = false
      hasAnimatedCover.value = false
    }

    // 3. Static HD cover: local sidecar first, then the remote artwork URL,
    //    then whatever the queue projection already had.
    if (metaResult?.hd_cover_path) {
      hdCoverUrl.value = await displayableUrl(metaResult.hd_cover_path)
    } else {
      hdCoverUrl.value =
        metaResult?.cover_art_url || current.value.coverUrl || null
    }

    // 4. Raw container tags (only for downloaded files; honest failure = empty).
    fileTags.value = {}
    if (metaResult?.file_path) {
      try {
        const snapshot = await readTrackTags(trackId)
        if (current.value?.id === trackId) {
          fileTags.value = snapshot?.all_tags ?? {}
        }
      } catch {
        fileTags.value = {}
      }
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

/**
 * Opens the booklet with the system's default PDF viewer.
 *
 * `window.open` on an `asset:` URL is unreliable (the webview is expected to
 * navigate to a non-http scheme), so the native command is the primary route.
 * The `window.open` fallback keeps the button useful in environments where the
 * command is unavailable (dev web preview, tests).
 */
async function openBookletExternal() {
  const path = bookletPath.value
  if (!path) return
  try {
    await invokeCommand('open_local_sidecar', { path })
    return
  } catch (err) {
    console.warn('[FullscreenPlayer] open_local_sidecar failed, falling back:', err)
  }
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
  revokeSidecarUrls()
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
