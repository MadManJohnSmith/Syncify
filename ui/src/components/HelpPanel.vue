<template>
  <div class="help-system">
    <!-- Help Panel -->
    <Teleport to="body">
      <Transition name="slide">
        <div v-if="isOpen" class="help-panel-overlay fixed inset-0 z-[150]" @click.self="close">
          <div class="help-panel absolute right-0 top-0 bottom-0 w-[400px] bg-surface shadow-2xl flex flex-col">
            
            <!-- Header -->
            <div class="help-header px-5 py-4 border-b border-line shrink-0">
              <div class="flex items-center justify-between mb-4">
                <h2 class="text-lg font-bold text-ink">Help & Support</h2>
                <button @click="close" class="p-2 hover:bg-elevated rounded-lg transition-colors">
                  <span class="material-symbols-outlined text-ink-3">close</span>
                </button>
              </div>
              
              <!-- Search -->
              <div class="relative">
                <span class="material-symbols-outlined absolute left-3 top-1/2 -translate-y-1/2 text-ink-3 text-[20px]">search</span>
                <input 
                  v-model="searchQuery"
                  type="text"
                  placeholder="Search help articles..."
                  class="w-full pl-10 pr-4 py-2.5 bg-elevated rounded-xl text-ink placeholder:text-ink-3 focus:outline-none focus:ring-2 focus:ring-accent/50"
                >
              </div>
            </div>
            
            <!-- Tabs -->
            <div class="help-tabs flex border-b border-line shrink-0">
              <button 
                v-for="tab in tabs" 
                :key="tab.id"
                @click="activeTab = tab.id"
                :class="[
                  'flex-1 py-3 text-sm font-medium transition-colors relative',
                  activeTab === tab.id ? 'text-accent' : 'text-ink-2 hover:text-ink'
                ]"
              >
                {{ tab.label }}
                <div v-if="activeTab === tab.id" class="absolute bottom-0 left-0 right-0 h-0.5 bg-accent"></div>
              </button>
            </div>
            
            <!-- Content -->
            <div class="flex-1 overflow-y-auto custom-scrollbar">
              <!-- Articles Tab -->
              <div v-if="activeTab === 'articles' && !selectedArticle" class="help-articles p-4">
                <!-- Search Results -->
                <div v-if="searchQuery && searchResults.length > 0" class="space-y-2">
                  <p class="text-xs text-ink-3 mb-3">{{ searchResults.length }} results for "{{ searchQuery }}"</p>
                  <div 
                    v-for="result in searchResults" 
                    :key="result.id"
                    @click="openArticle(result)"
                    class="p-3 bg-elevated rounded-lg cursor-pointer hover:bg-accent-soft transition-colors"
                  >
                    <p class="text-sm font-medium text-ink" v-html="highlightMatch(result.title)"></p>
                    <p class="text-xs text-ink-2 mt-1">{{ result.category }}</p>
                  </div>
                </div>
                
                <!-- Categories -->
                <div v-else class="space-y-4">
                  <div v-for="category in articleCategories" :key="category.name" class="article-category">
                    <button 
                      @click="toggleCategory(category.name)"
                      class="w-full flex items-center justify-between py-2"
                    >
                      <span class="text-sm font-semibold text-ink-2">{{ category.name }}</span>
                      <span class="material-symbols-outlined text-ink-3 text-lg transition-transform" :class="{ 'rotate-180': !collapsedCategories.includes(category.name) }">
                        expand_more
                      </span>
                    </button>
                    
                    <Transition name="accordion">
                      <div v-if="!collapsedCategories.includes(category.name)" class="space-y-1 mt-1">
                        <div 
                          v-for="article in category.articles" 
                          :key="article.id"
                          @click="openArticle(article)"
                          class="p-3 hover:bg-elevated rounded-lg cursor-pointer transition-colors flex items-center gap-3"
                        >
                          <span class="material-symbols-outlined text-ink-3 text-lg">article</span>
                          <span class="text-sm text-ink-2">{{ article.title }}</span>
                        </div>
                      </div>
                    </Transition>
                  </div>
                </div>
              </div>
              
              <!-- Article View -->
              <div v-if="activeTab === 'articles' && selectedArticle" class="article-view">
                <div class="p-4 border-b border-line">
                  <button @click="selectedArticle = null" class="flex items-center gap-2 text-sm text-accent hover:underline mb-3">
                    <span class="material-symbols-outlined text-[18px]">arrow_back</span>
                    Back to articles
                  </button>
                  <h3 class="text-xl font-bold text-ink">{{ selectedArticle.title }}</h3>
                </div>
                
                <div class="p-5 prose prose-sm dark:prose-invert max-w-none">
                  <div v-html="sanitizeHtml(selectedArticle.content)"></div>
                </div>
                
                <!-- Feedback -->
                <div class="p-4 border-t border-line">
                  <p class="text-sm text-ink-2 mb-3">Was this helpful?</p>
                  <div v-if="articleFeedbackSent === null" class="flex gap-2">
                    <button
                      @click="submitArticleFeedback(true)"
                      :disabled="articleFeedbackSubmitting"
                      class="flex items-center gap-2 px-4 py-2 bg-ok/10 text-ok hover:bg-ok/20 rounded-lg text-sm transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                    >
                      <span class="material-symbols-outlined text-lg">thumb_up</span>
                      Yes
                    </button>
                    <button
                      @click="submitArticleFeedback(false)"
                      :disabled="articleFeedbackSubmitting"
                      class="flex items-center gap-2 px-4 py-2 bg-error/10 text-error hover:bg-error/20 rounded-lg text-sm transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                    >
                      <span class="material-symbols-outlined text-lg">thumb_down</span>
                      No
                    </button>
                  </div>
                  <p v-else class="text-sm text-ok flex items-center gap-1">
                    <span class="material-symbols-outlined text-lg">check_circle</span>
                    Thanks for your feedback!
                  </p>
                </div>
              </div>
              
              <!-- Tutorials Tab -->
              <div v-if="activeTab === 'tutorials'" class="help-tutorials p-4 space-y-4">
                <!-- Video Tutorials -->
                <div>
                  <h4 class="text-sm font-semibold text-ink-2 mb-3">Video Tutorials</h4>
                  <div class="space-y-3">
                    <div v-for="video in tutorials" :key="video.id" class="p-4 bg-elevated rounded-xl">
                      <div class="flex items-center gap-3">
                        <div class="w-16 h-10 rounded bg-elevated flex items-center justify-center shrink-0">
                          <span class="material-symbols-outlined text-ink-3">play_circle</span>
                        </div>
                        <div class="flex-1 min-w-0">
                          <p class="text-sm font-medium text-ink">{{ video.title }}</p>
                          <p class="text-xs text-ink-2">{{ video.duration }}</p>
                        </div>
                      </div>
                    </div>
                  </div>
                </div>
                
                <!-- Interactive Tutorials -->
                <div>
                  <h4 class="text-sm font-semibold text-ink-2 mb-3">Interactive Guides</h4>
                  <button @click="startGuidedTour" class="w-full p-4 bg-accent/10 border border-accent/20 rounded-xl text-left hover:bg-accent/20 transition-colors">
                    <div class="flex items-center gap-3">
                      <div class="w-10 h-10 rounded-lg bg-accent/20 flex items-center justify-center">
                        <span class="material-symbols-outlined text-accent">tour</span>
                      </div>
                      <div>
                        <p class="text-sm font-medium text-ink">Take a Guided Tour</p>
                        <p class="text-xs text-ink-2">Walk through key features step-by-step</p>
                      </div>
                    </div>
                  </button>
                </div>
              </div>
              
              <!-- FAQ Tab -->
              <div v-if="activeTab === 'faq'" class="help-faq p-4 space-y-2">
                <div v-for="faq in filteredFaqs" :key="faq.id" class="border border-line rounded-xl overflow-hidden">
                  <button 
                    @click="toggleFaq(faq.id)"
                    class="w-full p-4 text-left flex items-center justify-between hover:bg-elevated transition-colors"
                  >
                    <span class="text-sm font-medium text-ink pr-4" v-html="highlightMatch(faq.question)"></span>
                    <span class="material-symbols-outlined text-ink-3 shrink-0 transition-transform" :class="{ 'rotate-180': expandedFaqs.includes(faq.id) }">
                      expand_more
                    </span>
                  </button>
                  <Transition name="accordion">
                    <div v-if="expandedFaqs.includes(faq.id)" class="px-4 pb-4">
                      <p class="text-sm text-ink-2">{{ faq.answer }}</p>
                    </div>
                  </Transition>
                </div>
              </div>
              
              <!-- Contact Tab -->
              <div v-if="activeTab === 'contact'" class="help-contact p-4 space-y-4">
                <!-- Support Options -->
                <div class="grid grid-cols-2 gap-3">
                  <button @click="openBugReport" class="p-4 bg-elevated rounded-xl hover:bg-accent-soft transition-colors text-left">
                    <span class="material-symbols-outlined text-error text-2xl mb-2 block">bug_report</span>
                    <p class="text-sm font-medium text-ink">Report a Bug</p>
                    <p class="text-xs text-ink-2 mt-1">Found an issue?</p>
                  </button>

                  <button @click="openFeedback" class="p-4 bg-elevated rounded-xl hover:bg-accent-soft transition-colors text-left">
                    <span class="material-symbols-outlined text-info text-2xl mb-2 block">chat</span>
                    <p class="text-sm font-medium text-ink">Send Feedback</p>
                    <p class="text-xs text-ink-2 mt-1">Share your thoughts</p>
                  </button>
                </div>
                
                <!-- Report destination -->
                <p class="text-xs text-ink-2">
                  Reports and feedback are written as files under your Syncify reports folder;
                  the path is shown as soon as the file is saved.
                </p>
                
                <!-- System Info -->
                <div class="mt-4">
                  <button @click="showSystemInfo = !showSystemInfo" class="flex items-center gap-2 text-sm text-ink-2 hover:text-ink">
                    <span class="material-symbols-outlined text-lg">{{ showSystemInfo ? 'expand_less' : 'expand_more' }}</span>
                    System Information
                  </button>
                  <Transition name="accordion">
                    <div v-if="showSystemInfo" class="mt-3 p-4 bg-elevated rounded-xl">
                      <div class="space-y-2 text-xs font-mono">
                        <div class="flex justify-between">
                          <span class="text-ink-2">App Version</span>
                          <span class="text-ink-2">{{ systemInfo.appVersion }}</span>
                        </div>
                        <div class="flex justify-between">
                          <span class="text-ink-2">OS</span>
                          <span class="text-ink-2">{{ systemInfo.os }}</span>
                        </div>
                        <div class="flex justify-between">
                          <span class="text-ink-2">Services</span>
                          <span class="text-ink-2">{{ systemInfo.connectedServices }}</span>
                        </div>
                      </div>
                      <button @click="copySystemInfo" class="mt-3 text-xs text-accent hover:underline flex items-center gap-1">
                        <span class="material-symbols-outlined text-[14px]">{{ systemInfoCopied ? 'check' : 'content_copy' }}</span>
                        {{ systemInfoCopied ? 'Copied!' : 'Copy info' }}
                      </button>
                    </div>
                  </Transition>
                </div>
                
                <!-- What's New -->
                <button @click="showWhatsNew = true" class="w-full p-4 bg-accent/10 border border-accent/20 rounded-xl text-left hover:bg-accent/20 transition-colors mt-4">
                  <div class="flex items-center gap-3">
                    <span class="material-symbols-outlined text-accent">new_releases</span>
                    <div>
                      <p class="text-sm font-medium text-ink">What's New in {{ systemInfo.appVersion }}</p>
                      <p class="text-xs text-ink-2">See the latest features</p>
                    </div>
                  </div>
                </button>
              </div>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>
    
    <!-- Bug Report Modal -->
    <Teleport to="body">
      <Transition name="fade">
        <div v-if="showBugReport" class="fixed inset-0 bg-black/50 flex items-center justify-center z-[200] p-8" @click.self="showBugReport = false">
          <div class="bg-surface rounded-2xl w-full max-w-md overflow-hidden shadow-2xl">
            <div class="px-5 py-4 border-b border-line flex items-center justify-between">
              <h3 class="text-lg font-semibold text-ink">Report a Bug</h3>
              <button @click="showBugReport = false" class="p-2 hover:bg-elevated rounded-lg">
                <span class="material-symbols-outlined text-ink-3">close</span>
              </button>
            </div>
            <div v-if="bugSavedPath" class="p-5">
              <div class="flex items-start gap-3 p-4 bg-ok/10 border border-ok/30 rounded-xl">
                <span class="material-symbols-outlined text-ok">check_circle</span>
                <div class="min-w-0">
                  <p class="text-sm font-medium text-ink">Bug report saved</p>
                  <p class="text-xs text-ink-2 mt-1 break-all">{{ bugSavedPath }}</p>
                </div>
              </div>
            </div>
            <div v-else class="p-5 space-y-4">
              <div>
                <label class="block text-sm font-medium text-ink-2 mb-1">Description</label>
                <textarea v-model="bugDescription" rows="4" class="w-full px-3 py-2 bg-elevated rounded-lg text-ink focus:outline-none focus:ring-2 focus:ring-accent/50" placeholder="Describe the bug..."></textarea>
              </div>
              <div>
                <label class="block text-sm font-medium text-ink-2 mb-1">Steps to Reproduce</label>
                <textarea v-model="bugSteps" rows="3" class="w-full px-3 py-2 bg-elevated rounded-lg text-ink focus:outline-none focus:ring-2 focus:ring-accent/50" placeholder="1. Go to..."></textarea>
              </div>
              <label class="flex items-center gap-2 cursor-pointer">
                <input v-model="bugAttachLogs" type="checkbox" class="w-4 h-4 rounded border-line-strong text-accent focus:ring-accent">
                <span class="text-sm text-ink-2">Attach system logs</span>
              </label>
              <p v-if="bugError" class="text-sm text-error">{{ bugError }}</p>
            </div>
            <div class="px-5 py-4 border-t border-line flex justify-end gap-3">
              <button @click="showBugReport = false" class="px-4 py-2 text-ink-2 hover:bg-elevated rounded-lg">Cancel</button>
              <button
                v-if="!bugSavedPath"
                @click="submitBugReport"
                :disabled="bugSubmitting"
                class="px-4 py-2 bg-accent hover:bg-accent-hover text-on-accent rounded-lg font-medium disabled:opacity-50 disabled:cursor-not-allowed"
              >
                {{ bugSubmitting ? 'Saving...' : 'Submit Report' }}
              </button>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>
    
    <!-- Feedback Modal -->
    <Teleport to="body">
      <Transition name="fade">
        <div v-if="showFeedback" class="fixed inset-0 bg-black/50 flex items-center justify-center z-[200] p-8" @click.self="showFeedback = false">
          <div class="bg-surface rounded-2xl w-full max-w-md overflow-hidden shadow-2xl">
            <div class="px-5 py-4 border-b border-line flex items-center justify-between">
              <h3 class="text-lg font-semibold text-ink">Send Feedback</h3>
              <button @click="showFeedback = false" class="p-2 hover:bg-elevated rounded-lg">
                <span class="material-symbols-outlined text-ink-3">close</span>
              </button>
            </div>
            <div v-if="feedbackSavedPath" class="p-5">
              <div class="flex items-start gap-3 p-4 bg-ok/10 border border-ok/30 rounded-xl">
                <span class="material-symbols-outlined text-ok">check_circle</span>
                <div class="min-w-0">
                  <p class="text-sm font-medium text-ink">Feedback saved</p>
                  <p class="text-xs text-ink-2 mt-1 break-all">{{ feedbackSavedPath }}</p>
                </div>
              </div>
            </div>
            <div v-else class="p-5 space-y-4">
              <div>
                <label class="block text-sm font-medium text-ink-2 mb-1">Type</label>
                <select v-model="feedbackType" class="w-full px-3 py-2 bg-elevated rounded-lg text-ink focus:outline-none focus:ring-2 focus:ring-accent/50">
                  <option>Bug Report</option>
                  <option>Feature Request</option>
                  <option>General Feedback</option>
                </select>
              </div>
              <div>
                <label class="block text-sm font-medium text-ink-2 mb-1">Message</label>
                <textarea v-model="feedbackMessage" rows="4" class="w-full px-3 py-2 bg-elevated rounded-lg text-ink focus:outline-none focus:ring-2 focus:ring-accent/50" placeholder="Your feedback..."></textarea>
              </div>
              <p v-if="feedbackError" class="text-sm text-error">{{ feedbackError }}</p>
            </div>
            <div class="px-5 py-4 border-t border-line flex justify-end gap-3">
              <button @click="showFeedback = false" class="px-4 py-2 text-ink-2 hover:bg-elevated rounded-lg">Cancel</button>
              <button
                v-if="!feedbackSavedPath"
                @click="submitFeedback"
                :disabled="feedbackSubmitting"
                class="px-4 py-2 bg-accent hover:bg-accent-hover text-on-accent rounded-lg font-medium disabled:opacity-50 disabled:cursor-not-allowed"
              >
                {{ feedbackSubmitting ? 'Saving...' : 'Send Feedback' }}
              </button>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>
    
    <!-- What's New Modal -->
    <Teleport to="body">
      <Transition name="fade">
        <div v-if="showWhatsNew" class="whats-new fixed inset-0 bg-black/50 flex items-center justify-center z-[200] p-8" @click.self="showWhatsNew = false">
          <div class="bg-surface rounded-2xl w-full max-w-lg overflow-hidden shadow-2xl">
            <div class="px-5 py-4 border-b border-line flex items-center justify-between">
              <div>
                <h3 class="text-lg font-semibold text-ink">What's New</h3>
                <p class="text-sm text-ink-2">Version {{ systemInfo.appVersion }}</p>
              </div>
              <button @click="showWhatsNew = false" class="p-2 hover:bg-elevated rounded-lg">
                <span class="material-symbols-outlined text-ink-3">close</span>
              </button>
            </div>
            <div class="p-5 max-h-96 overflow-y-auto space-y-4">
              <div>
                <h4 class="text-sm font-semibold text-ok mb-2 flex items-center gap-2">
                  <span class="material-symbols-outlined text-lg">add_circle</span>
                  New Features
                </h4>
                <ul class="space-y-2">
                  <li class="text-sm text-ink-2 flex items-start gap-2">
                    <span class="text-ok mt-1">•</span>
                    Advanced lyrics sync editor with waveform display
                  </li>
                  <li class="text-sm text-ink-2 flex items-start gap-2">
                    <span class="text-ok mt-1">•</span>
                    Command palette for quick navigation (Ctrl+K)
                  </li>
                  <li class="text-sm text-ink-2 flex items-start gap-2">
                    <span class="text-ok mt-1">•</span>
                    Keyboard shortcuts help modal
                  </li>
                </ul>
              </div>
              <div>
                <h4 class="text-sm font-semibold text-info mb-2 flex items-center gap-2">
                  <span class="material-symbols-outlined text-lg">upgrade</span>
                  Improvements
                </h4>
                <ul class="space-y-2">
                  <li class="text-sm text-ink-2 flex items-start gap-2">
                    <span class="text-info mt-1">•</span>
                    Faster library loading with virtual scrolling
                  </li>
                  <li class="text-sm text-ink-2 flex items-start gap-2">
                    <span class="text-info mt-1">•</span>
                    Improved metadata matching accuracy
                  </li>
                </ul>
              </div>
              <div>
                <h4 class="text-sm font-semibold text-warn mb-2 flex items-center gap-2">
                  <span class="material-symbols-outlined text-lg">bug_report</span>
                  Bug Fixes
                </h4>
                <ul class="space-y-2">
                  <li class="text-sm text-ink-2 flex items-start gap-2">
                    <span class="text-warn mt-1">•</span>
                    Fixed download resume issues
                  </li>
                  <li class="text-sm text-ink-2 flex items-start gap-2">
                    <span class="text-warn mt-1">•</span>
                    Resolved memory leak in library view
                  </li>
                </ul>
              </div>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted } from 'vue'
import { getVersion } from '@tauri-apps/api/app'
import { escapeHtml, escapeRegex, sanitizeHtml, highlightMatch as safeHighlightMatch } from '@/utils/sanitize'
import { toolsApi } from '@/api/tools'
import { getServiceStatuses } from '@/api/accounts'
import { useToast } from '../composables/useToast'

const props = withDefaults(
  defineProps<{
    modelValue?: boolean
  }>(),
  {
    modelValue: undefined,
  }
)

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void
  (e: 'close'): void
}>()

// State
const internalOpen = ref(false)
const isOpen = computed({
  get: () => (props.modelValue !== undefined ? props.modelValue : internalOpen.value),
  set: (val: boolean) => {
    internalOpen.value = val
    emit('update:modelValue', val)
    if (!val) {
      emit('close')
    }
  },
})

const searchQuery = ref('')
const activeTab = ref('articles')
const collapsedCategories = ref<string[]>([])
const selectedArticle = ref<any>(null)
const expandedFaqs = ref<number[]>([])
const showSystemInfo = ref(false)
const showBugReport = ref(false)
const showFeedback = ref(false)
const showWhatsNew = ref(false)

// Tabs
const tabs = [
  { id: 'articles', label: 'Articles' },
  { id: 'tutorials', label: 'Tutorials' },
  { id: 'faq', label: 'FAQ' },
  { id: 'contact', label: 'Contact' },
]

// Article categories
const articleCategories = ref([
  {
    name: 'Getting Started',
    articles: [
      { id: 1, title: 'Setting up your first service', category: 'Getting Started', content: '<p>Learn how to connect your streaming services...</p>' },
      { id: 2, title: 'Importing your music library', category: 'Getting Started', content: '<p>Import tracks from connected services...</p>' },
      { id: 3, title: 'Downloading tracks', category: 'Getting Started', content: '<p>How to download music to your device...</p>' },
      { id: 4, title: 'Understanding quality settings', category: 'Getting Started', content: '<p>Audio quality options explained...</p>' },
    ]
  },
  {
    name: 'Library Management',
    articles: [
      { id: 5, title: 'Organizing your library', category: 'Library Management', content: '<p>Tips for organizing your music...</p>' },
      { id: 6, title: 'Managing playlists', category: 'Library Management', content: '<p>Create and manage playlists...</p>' },
      { id: 7, title: 'Using filters and search', category: 'Library Management', content: '<p>Find tracks quickly...</p>' },
      { id: 8, title: 'Batch operations', category: 'Library Management', content: '<p>Edit multiple tracks at once...</p>' },
    ]
  },
  {
    name: 'Downloads',
    articles: [
      { id: 9, title: 'Managing download queue', category: 'Downloads', content: '<p>Control your download queue...</p>' },
      { id: 10, title: 'Retry failed downloads', category: 'Downloads', content: '<p>How to handle failed downloads...</p>' },
      { id: 11, title: 'Setting download priorities', category: 'Downloads', content: '<p>Prioritize important downloads...</p>' },
    ]
  },
  {
    name: 'Troubleshooting',
    articles: [
      { id: 12, title: 'Connection issues', category: 'Troubleshooting', content: '<p>Fixing connection problems...</p>' },
      { id: 13, title: 'Authentication problems', category: 'Troubleshooting', content: '<p>Login and auth issues...</p>' },
      { id: 14, title: 'Download failures', category: 'Troubleshooting', content: '<p>Why downloads might fail...</p>' },
    ]
  },
])

// All articles flat
const allArticles = computed(() => articleCategories.value.flatMap(c => c.articles))

// Search results
const searchResults = computed(() => {
  if (!searchQuery.value.trim()) return []
  const query = searchQuery.value.toLowerCase()
  return allArticles.value.filter(a => 
    a.title.toLowerCase().includes(query)
  )
})

// Tutorials
const tutorials = ref([
  { id: 1, title: 'Quick start: 5-minute overview', duration: '5:00' },
  { id: 2, title: 'Connecting services', duration: '3:30' },
  { id: 3, title: 'Advanced search and filtering', duration: '4:15' },
  { id: 4, title: 'Migration walkthrough', duration: '8:00' },
])

// FAQs
const faqs = ref([
  { id: 1, question: 'How do I add multiple accounts for the same service?', answer: 'Open the Accounts view and click "Add Connection" for any service. You can have multiple accounts per service.' },
  { id: 2, question: 'What audio formats are supported?', answer: 'Syncify supports FLAC, ALAC, WAV, MP3, AAC, and OGG formats. Hi-Res audio up to 24-bit/192kHz is supported.' },
  { id: 3, question: 'How does matching work between services?', answer: 'Matching uses ISRC codes, MusicBrainz IDs, and fuzzy title/artist matching to find equivalent tracks across services.' },
  { id: 4, question: 'Can I sync favorites automatically?', answer: 'Yes! In Settings > Sync, switch on "Sync favorites" for every service you want to keep in sync, then run the sync from the Dashboard or from the command palette.' },
  { id: 5, question: 'How much storage do I need?', answer: 'Storage depends on quality settings. Hi-Res FLAC uses ~100MB per album, CD quality ~50MB, and MP3 ~10MB.' },
  { id: 6, question: 'Is my data encrypted?', answer: 'Yes, all credentials are encrypted using AES-256 and stored locally. No data is sent to external servers.' },
])

const filteredFaqs = computed(() => {
  if (!searchQuery.value.trim()) return faqs.value
  const query = searchQuery.value.toLowerCase()
  return faqs.value.filter(f => 
    f.question.toLowerCase().includes(query) || 
    f.answer.toLowerCase().includes(query)
  )
})

// System info
const systemInfo = ref({
  appVersion: 'unknown',
  os: detectOsLabel(),
  connectedServices: '…'
})

// Report / feedback form state (FE-8)
const toast = useToast()
const bugDescription = ref('')
const bugSteps = ref('')
const bugAttachLogs = ref(true)
const bugSubmitting = ref(false)
const bugError = ref('')
const bugSavedPath = ref('')

const feedbackType = ref('General Feedback')
const feedbackMessage = ref('')
const feedbackSubmitting = ref(false)
const feedbackError = ref('')
const feedbackSavedPath = ref('')

const articleFeedbackSent = ref<null | boolean>(null)
const articleFeedbackSubmitting = ref(false)

const systemInfoCopied = ref(false)
let copiedTimer: ReturnType<typeof setTimeout> | undefined

function detectOsLabel(): string {
  const ua = typeof navigator !== 'undefined' ? navigator.userAgent : ''
  if (/Windows/i.test(ua)) return 'Windows'
  if (/Android/i.test(ua)) return 'Android'
  if (/iPhone|iPad|iPod/i.test(ua)) return 'iOS'
  if (/Mac OS X|Macintosh/i.test(ua)) return 'macOS'
  if (/Linux/i.test(ua)) return 'Linux'
  return 'Unknown'
}

/** Load real app version and connected services for the System Info block */
async function loadSystemInfo() {
  try {
    const version = await getVersion()
    if (version) systemInfo.value.appVersion = version
  } catch {
    // Non-Tauri environment: keep the placeholder
  }
  try {
    const statuses = await getServiceStatuses()
    const connected = statuses.filter(s => s.connected)
    systemInfo.value.connectedServices = connected.length > 0
      ? `${connected.length} (${connected.map(s => s.name).join(', ')})`
      : 'None connected'
  } catch {
    // Backend unavailable: keep the placeholder
  }
}

function openBugReport() {
  bugDescription.value = ''
  bugSteps.value = ''
  bugAttachLogs.value = true
  bugError.value = ''
  bugSavedPath.value = ''
  showBugReport.value = true
}

function openFeedback() {
  feedbackType.value = 'General Feedback'
  feedbackMessage.value = ''
  feedbackError.value = ''
  feedbackSavedPath.value = ''
  showFeedback.value = true
}

async function submitBugReport() {
  bugError.value = ''
  if (!bugDescription.value.trim()) {
    bugError.value = 'Please describe the bug before submitting.'
    return
  }
  bugSubmitting.value = true
  try {
    const saved = await toolsApi.saveUserReport({
      kind: 'bug_report',
      message: bugDescription.value.trim(),
      steps_to_reproduce: bugSteps.value.trim(),
      attach_logs: bugAttachLogs.value,
    })
    bugSavedPath.value = saved.filePath
    toast.success('Bug report saved locally')
  } catch (e) {
    bugError.value = e instanceof Error ? e.message : String(e)
    toast.error('Failed to save bug report')
  } finally {
    bugSubmitting.value = false
  }
}

async function submitFeedback() {
  feedbackError.value = ''
  if (!feedbackMessage.value.trim()) {
    feedbackError.value = 'Please write your feedback before sending.'
    return
  }
  feedbackSubmitting.value = true
  try {
    const saved = await toolsApi.saveUserReport({
      kind: 'feedback',
      message: feedbackMessage.value.trim(),
      feedback_type: feedbackType.value,
    })
    feedbackSavedPath.value = saved.filePath
    toast.success('Feedback saved locally')
  } catch (e) {
    feedbackError.value = e instanceof Error ? e.message : String(e)
    toast.error('Failed to save feedback')
  } finally {
    feedbackSubmitting.value = false
  }
}

async function submitArticleFeedback(helpful: boolean) {
  if (articleFeedbackSubmitting.value || articleFeedbackSent.value !== null) return
  articleFeedbackSubmitting.value = true
  try {
    await toolsApi.saveUserReport({
      kind: 'article_feedback',
      message: '',
      article_title: selectedArticle.value?.title ?? '',
      helpful,
    })
    articleFeedbackSent.value = helpful
  } catch {
    toast.error('Failed to save your feedback')
  } finally {
    articleFeedbackSubmitting.value = false
  }
}

async function copySystemInfo() {
  const lines = [
    `App Version: ${systemInfo.value.appVersion}`,
    `OS: ${systemInfo.value.os}`,
    `Connected Services: ${systemInfo.value.connectedServices}`,
  ].join('\n')
  try {
    await navigator.clipboard.writeText(lines)
    systemInfoCopied.value = true
    if (copiedTimer) clearTimeout(copiedTimer)
    copiedTimer = setTimeout(() => { systemInfoCopied.value = false }, 2000)
  } catch {
    toast.error('Could not copy to clipboard')
  }
}

// A fresh article gets a fresh "Was this helpful?" state
watch(selectedArticle, () => {
  articleFeedbackSent.value = null
  articleFeedbackSubmitting.value = false
})

// Methods
function toggleCategory(name: string) {
  const idx = collapsedCategories.value.indexOf(name)
  if (idx === -1) collapsedCategories.value.push(name)
  else collapsedCategories.value.splice(idx, 1)
}

function toggleFaq(id: number) {
  const idx = expandedFaqs.value.indexOf(id)
  if (idx === -1) expandedFaqs.value.push(id)
  else expandedFaqs.value.splice(idx, 1)
}

function openArticle(article: any) {
  selectedArticle.value = article
}

function highlightMatch(text: string): string {
  return safeHighlightMatch(text, searchQuery.value)
}

function startGuidedTour() {
  close()
  // Emit event to start tour
}

function open() {
  isOpen.value = true
}

function close() {
  isOpen.value = false
  selectedArticle.value = null
}

watch(isOpen, (val) => {
  if (!val) {
    selectedArticle.value = null
  }
})

// Global keyboard listener
function handleKeydown(event: KeyboardEvent) {
  if ((event.ctrlKey && event.key === 'h') || event.key === 'F1') {
    event.preventDefault()
    isOpen.value ? close() : open()
  }
  if (event.key === 'Escape' && isOpen.value) {
    event.preventDefault()
    close()
  }
}

onMounted(() => {
  document.addEventListener('keydown', handleKeydown)
  loadSystemInfo()
})

onUnmounted(() => {
  document.removeEventListener('keydown', handleKeydown)
  if (copiedTimer) {
    clearTimeout(copiedTimer)
    copiedTimer = undefined
  }
})

defineExpose({ open, close, isOpen })
</script>

<style scoped>
/* Panel slide animation */
.slide-enter-active,
.slide-leave-active {
  transition: all 0.25s ease;
}

.slide-enter-from .help-panel,
.slide-leave-to .help-panel {
  transform: translateX(100%);
}

.help-panel-overlay {
  background: rgba(0, 0, 0, 0.4);
}

.slide-enter-from,
.slide-leave-to {
  opacity: 0;
}

/* Fade transition */
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.2s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

/* Accordion transition */
.accordion-enter-active,
.accordion-leave-active {
  transition: all 0.2s ease;
  overflow: hidden;
}

.accordion-enter-from,
.accordion-leave-to {
  opacity: 0;
  max-height: 0;
}

/* Custom scrollbar */
.custom-scrollbar::-webkit-scrollbar {
  width: 6px;
}

.custom-scrollbar::-webkit-scrollbar-track {
  background: transparent;
}

.custom-scrollbar::-webkit-scrollbar-thumb {
  background: rgba(0, 0, 0, 0.1);
  border-radius: 3px;
}

.dark .custom-scrollbar::-webkit-scrollbar-thumb {
  background: rgba(255, 255, 255, 0.1);
}

/* Prose styling */
.prose {
  line-height: 1.7;
}

.prose p {
  margin-bottom: 1em;
}
</style>
