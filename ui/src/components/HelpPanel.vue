<template>
  <div class="help-system">
    <!-- Help Panel -->
    <Teleport to="body">
      <Transition name="slide">
        <div v-if="isOpen" class="help-panel-overlay fixed inset-0 z-[150]" @click.self="close">
          <div class="help-panel absolute right-0 top-0 bottom-0 w-[400px] bg-white dark:bg-surface-dark shadow-2xl flex flex-col">
            
            <!-- Header -->
            <div class="help-header px-5 py-4 border-b border-gray-200 dark:border-border-dark shrink-0">
              <div class="flex items-center justify-between mb-4">
                <h2 class="text-lg font-bold text-gray-900 dark:text-white">Help & Support</h2>
                <button @click="close" class="p-2 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-lg transition-colors">
                  <span class="material-symbols-outlined text-gray-400">close</span>
                </button>
              </div>
              
              <!-- Search -->
              <div class="relative">
                <span class="material-symbols-outlined absolute left-3 top-1/2 -translate-y-1/2 text-gray-400 text-[20px]">search</span>
                <input 
                  v-model="searchQuery"
                  type="text"
                  placeholder="Search help articles..."
                  class="w-full pl-10 pr-4 py-2.5 bg-gray-100 dark:bg-surface-highlight rounded-xl text-gray-900 dark:text-white placeholder-gray-400 focus:outline-none focus:ring-2 focus:ring-primary/50"
                >
              </div>
            </div>
            
            <!-- Tabs -->
            <div class="help-tabs flex border-b border-gray-200 dark:border-border-dark shrink-0">
              <button 
                v-for="tab in tabs" 
                :key="tab.id"
                @click="activeTab = tab.id"
                :class="[
                  'flex-1 py-3 text-sm font-medium transition-colors relative',
                  activeTab === tab.id ? 'text-primary' : 'text-gray-500 hover:text-gray-700 dark:hover:text-gray-300'
                ]"
              >
                {{ tab.label }}
                <div v-if="activeTab === tab.id" class="absolute bottom-0 left-0 right-0 h-0.5 bg-primary"></div>
              </button>
            </div>
            
            <!-- Content -->
            <div class="flex-1 overflow-y-auto custom-scrollbar">
              <!-- Articles Tab -->
              <div v-if="activeTab === 'articles' && !selectedArticle" class="help-articles p-4">
                <!-- Search Results -->
                <div v-if="searchQuery && searchResults.length > 0" class="space-y-2">
                  <p class="text-xs text-gray-400 mb-3">{{ searchResults.length }} results for "{{ searchQuery }}"</p>
                  <div 
                    v-for="result in searchResults" 
                    :key="result.id"
                    @click="openArticle(result)"
                    class="p-3 bg-gray-50 dark:bg-surface-highlight rounded-lg cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors"
                  >
                    <p class="text-sm font-medium text-gray-900 dark:text-white" v-html="highlightMatch(result.title)"></p>
                    <p class="text-xs text-gray-500 mt-1">{{ result.category }}</p>
                  </div>
                </div>
                
                <!-- Categories -->
                <div v-else class="space-y-4">
                  <div v-for="category in articleCategories" :key="category.name" class="article-category">
                    <button 
                      @click="toggleCategory(category.name)"
                      class="w-full flex items-center justify-between py-2"
                    >
                      <span class="text-sm font-semibold text-gray-700 dark:text-gray-300">{{ category.name }}</span>
                      <span class="material-symbols-outlined text-gray-400 text-lg transition-transform" :class="{ 'rotate-180': !collapsedCategories.includes(category.name) }">
                        expand_more
                      </span>
                    </button>
                    
                    <Transition name="accordion">
                      <div v-if="!collapsedCategories.includes(category.name)" class="space-y-1 mt-1">
                        <div 
                          v-for="article in category.articles" 
                          :key="article.id"
                          @click="openArticle(article)"
                          class="p-3 hover:bg-gray-50 dark:hover:bg-surface-highlight rounded-lg cursor-pointer transition-colors flex items-center gap-3"
                        >
                          <span class="material-symbols-outlined text-gray-400 text-lg">article</span>
                          <span class="text-sm text-gray-600 dark:text-gray-400">{{ article.title }}</span>
                        </div>
                      </div>
                    </Transition>
                  </div>
                </div>
              </div>
              
              <!-- Article View -->
              <div v-if="activeTab === 'articles' && selectedArticle" class="article-view">
                <div class="p-4 border-b border-gray-200 dark:border-border-dark">
                  <button @click="selectedArticle = null" class="flex items-center gap-2 text-sm text-primary hover:underline mb-3">
                    <span class="material-symbols-outlined text-[18px]">arrow_back</span>
                    Back to articles
                  </button>
                  <h3 class="text-xl font-bold text-gray-900 dark:text-white">{{ selectedArticle.title }}</h3>
                </div>
                
                <div class="p-5 prose prose-sm dark:prose-invert max-w-none">
                  <div v-html="selectedArticle.content"></div>
                </div>
                
                <!-- Feedback -->
                <div class="p-4 border-t border-gray-200 dark:border-border-dark">
                  <p class="text-sm text-gray-500 mb-3">Was this helpful?</p>
                  <div class="flex gap-2">
                    <button class="flex items-center gap-2 px-4 py-2 bg-green-500/10 text-green-600 hover:bg-green-500/20 rounded-lg text-sm transition-colors">
                      <span class="material-symbols-outlined text-lg">thumb_up</span>
                      Yes
                    </button>
                    <button class="flex items-center gap-2 px-4 py-2 bg-red-500/10 text-red-500 hover:bg-red-500/20 rounded-lg text-sm transition-colors">
                      <span class="material-symbols-outlined text-lg">thumb_down</span>
                      No
                    </button>
                  </div>
                </div>
              </div>
              
              <!-- Tutorials Tab -->
              <div v-if="activeTab === 'tutorials'" class="help-tutorials p-4 space-y-4">
                <!-- Video Tutorials -->
                <div>
                  <h4 class="text-sm font-semibold text-gray-700 dark:text-gray-300 mb-3">Video Tutorials</h4>
                  <div class="space-y-3">
                    <div v-for="video in tutorials" :key="video.id" class="p-4 bg-gray-50 dark:bg-surface-highlight rounded-xl">
                      <div class="flex items-center gap-3">
                        <div class="w-16 h-10 rounded bg-gray-200 dark:bg-gray-700 flex items-center justify-center shrink-0">
                          <span class="material-symbols-outlined text-gray-400">play_circle</span>
                        </div>
                        <div class="flex-1 min-w-0">
                          <p class="text-sm font-medium text-gray-900 dark:text-white">{{ video.title }}</p>
                          <p class="text-xs text-gray-500">{{ video.duration }}</p>
                        </div>
                      </div>
                    </div>
                  </div>
                </div>
                
                <!-- Interactive Tutorials -->
                <div>
                  <h4 class="text-sm font-semibold text-gray-700 dark:text-gray-300 mb-3">Interactive Guides</h4>
                  <button @click="startGuidedTour" class="w-full p-4 bg-primary/10 border border-primary/20 rounded-xl text-left hover:bg-primary/20 transition-colors">
                    <div class="flex items-center gap-3">
                      <div class="w-10 h-10 rounded-lg bg-primary/20 flex items-center justify-center">
                        <span class="material-symbols-outlined text-primary">tour</span>
                      </div>
                      <div>
                        <p class="text-sm font-medium text-gray-900 dark:text-white">Take a Guided Tour</p>
                        <p class="text-xs text-gray-500">Walk through key features step-by-step</p>
                      </div>
                    </div>
                  </button>
                </div>
              </div>
              
              <!-- FAQ Tab -->
              <div v-if="activeTab === 'faq'" class="help-faq p-4 space-y-2">
                <div v-for="faq in filteredFaqs" :key="faq.id" class="border border-gray-200 dark:border-border-dark rounded-xl overflow-hidden">
                  <button 
                    @click="toggleFaq(faq.id)"
                    class="w-full p-4 text-left flex items-center justify-between hover:bg-gray-50 dark:hover:bg-surface-highlight transition-colors"
                  >
                    <span class="text-sm font-medium text-gray-900 dark:text-white pr-4" v-html="highlightMatch(faq.question)"></span>
                    <span class="material-symbols-outlined text-gray-400 shrink-0 transition-transform" :class="{ 'rotate-180': expandedFaqs.includes(faq.id) }">
                      expand_more
                    </span>
                  </button>
                  <Transition name="accordion">
                    <div v-if="expandedFaqs.includes(faq.id)" class="px-4 pb-4">
                      <p class="text-sm text-gray-600 dark:text-gray-400">{{ faq.answer }}</p>
                    </div>
                  </Transition>
                </div>
              </div>
              
              <!-- Contact Tab -->
              <div v-if="activeTab === 'contact'" class="help-contact p-4 space-y-4">
                <!-- Support Options -->
                <div class="grid grid-cols-2 gap-3">
                  <button @click="showBugReport = true" class="p-4 bg-gray-50 dark:bg-surface-highlight rounded-xl hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors text-left">
                    <span class="material-symbols-outlined text-red-500 text-2xl mb-2 block">bug_report</span>
                    <p class="text-sm font-medium text-gray-900 dark:text-white">Report a Bug</p>
                    <p class="text-xs text-gray-500 mt-1">Found an issue?</p>
                  </button>
                  
                  <button @click="showFeedback = true" class="p-4 bg-gray-50 dark:bg-surface-highlight rounded-xl hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors text-left">
                    <span class="material-symbols-outlined text-blue-500 text-2xl mb-2 block">chat</span>
                    <p class="text-sm font-medium text-gray-900 dark:text-white">Send Feedback</p>
                    <p class="text-xs text-gray-500 mt-1">Share your thoughts</p>
                  </button>
                </div>
                
                <!-- External Links -->
                <div class="space-y-2">
                  <a href="#" class="flex items-center gap-3 p-3 hover:bg-gray-50 dark:hover:bg-surface-highlight rounded-lg transition-colors">
                    <span class="material-symbols-outlined text-gray-400">forum</span>
                    <span class="text-sm text-gray-700 dark:text-gray-300">Community Forum</span>
                    <span class="material-symbols-outlined text-gray-300 text-lg ml-auto">open_in_new</span>
                  </a>
                  <a href="#" class="flex items-center gap-3 p-3 hover:bg-gray-50 dark:hover:bg-surface-highlight rounded-lg transition-colors">
                    <span class="material-symbols-outlined text-gray-400">mail</span>
                    <span class="text-sm text-gray-700 dark:text-gray-300">Email Support</span>
                    <span class="material-symbols-outlined text-gray-300 text-lg ml-auto">open_in_new</span>
                  </a>
                  <a href="#" class="flex items-center gap-3 p-3 hover:bg-gray-50 dark:hover:bg-surface-highlight rounded-lg transition-colors">
                    <span class="material-symbols-outlined text-gray-400">chat_bubble</span>
                    <span class="text-sm text-gray-700 dark:text-gray-300">Join Discord</span>
                    <span class="material-symbols-outlined text-gray-300 text-lg ml-auto">open_in_new</span>
                  </a>
                </div>
                
                <!-- System Info -->
                <div class="mt-4">
                  <button @click="showSystemInfo = !showSystemInfo" class="flex items-center gap-2 text-sm text-gray-500 hover:text-gray-700 dark:hover:text-gray-300">
                    <span class="material-symbols-outlined text-lg">{{ showSystemInfo ? 'expand_less' : 'expand_more' }}</span>
                    System Information
                  </button>
                  <Transition name="accordion">
                    <div v-if="showSystemInfo" class="mt-3 p-4 bg-gray-50 dark:bg-surface-highlight rounded-xl">
                      <div class="space-y-2 text-xs font-mono">
                        <div class="flex justify-between">
                          <span class="text-gray-500">App Version</span>
                          <span class="text-gray-700 dark:text-gray-300">{{ systemInfo.appVersion }}</span>
                        </div>
                        <div class="flex justify-between">
                          <span class="text-gray-500">OS</span>
                          <span class="text-gray-700 dark:text-gray-300">{{ systemInfo.os }}</span>
                        </div>
                        <div class="flex justify-between">
                          <span class="text-gray-500">Services</span>
                          <span class="text-gray-700 dark:text-gray-300">{{ systemInfo.connectedServices }}</span>
                        </div>
                      </div>
                      <button class="mt-3 text-xs text-primary hover:underline flex items-center gap-1">
                        <span class="material-symbols-outlined text-[14px]">content_copy</span>
                        Copy info
                      </button>
                    </div>
                  </Transition>
                </div>
                
                <!-- What's New -->
                <button @click="showWhatsNew = true" class="w-full p-4 bg-primary/10 border border-primary/20 rounded-xl text-left hover:bg-primary/20 transition-colors mt-4">
                  <div class="flex items-center gap-3">
                    <span class="material-symbols-outlined text-primary">new_releases</span>
                    <div>
                      <p class="text-sm font-medium text-gray-900 dark:text-white">What's New in v2.1.0</p>
                      <p class="text-xs text-gray-500">See the latest features</p>
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
          <div class="bg-white dark:bg-surface-dark rounded-2xl w-full max-w-md overflow-hidden shadow-2xl">
            <div class="px-5 py-4 border-b border-gray-200 dark:border-border-dark flex items-center justify-between">
              <h3 class="text-lg font-semibold text-gray-900 dark:text-white">Report a Bug</h3>
              <button @click="showBugReport = false" class="p-2 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-lg">
                <span class="material-symbols-outlined text-gray-400">close</span>
              </button>
            </div>
            <div class="p-5 space-y-4">
              <div>
                <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Description</label>
                <textarea rows="4" class="w-full px-3 py-2 bg-gray-100 dark:bg-surface-highlight rounded-lg text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary/50" placeholder="Describe the bug..."></textarea>
              </div>
              <div>
                <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Steps to Reproduce</label>
                <textarea rows="3" class="w-full px-3 py-2 bg-gray-100 dark:bg-surface-highlight rounded-lg text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary/50" placeholder="1. Go to..."></textarea>
              </div>
              <label class="flex items-center gap-2 cursor-pointer">
                <input type="checkbox" checked class="w-4 h-4 rounded border-gray-300 text-primary focus:ring-primary">
                <span class="text-sm text-gray-600 dark:text-gray-400">Attach system logs</span>
              </label>
            </div>
            <div class="px-5 py-4 border-t border-gray-200 dark:border-border-dark flex justify-end gap-3">
              <button @click="showBugReport = false" class="px-4 py-2 text-gray-600 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-lg">Cancel</button>
              <button class="px-4 py-2 bg-primary hover:bg-primary-hover text-white rounded-lg font-medium">Submit Report</button>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>
    
    <!-- Feedback Modal -->
    <Teleport to="body">
      <Transition name="fade">
        <div v-if="showFeedback" class="fixed inset-0 bg-black/50 flex items-center justify-center z-[200] p-8" @click.self="showFeedback = false">
          <div class="bg-white dark:bg-surface-dark rounded-2xl w-full max-w-md overflow-hidden shadow-2xl">
            <div class="px-5 py-4 border-b border-gray-200 dark:border-border-dark flex items-center justify-between">
              <h3 class="text-lg font-semibold text-gray-900 dark:text-white">Send Feedback</h3>
              <button @click="showFeedback = false" class="p-2 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-lg">
                <span class="material-symbols-outlined text-gray-400">close</span>
              </button>
            </div>
            <div class="p-5 space-y-4">
              <div>
                <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Type</label>
                <select class="w-full px-3 py-2 bg-gray-100 dark:bg-surface-highlight rounded-lg text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary/50">
                  <option>Bug Report</option>
                  <option>Feature Request</option>
                  <option>General Feedback</option>
                </select>
              </div>
              <div>
                <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Message</label>
                <textarea rows="4" class="w-full px-3 py-2 bg-gray-100 dark:bg-surface-highlight rounded-lg text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary/50" placeholder="Your feedback..."></textarea>
              </div>
            </div>
            <div class="px-5 py-4 border-t border-gray-200 dark:border-border-dark flex justify-end gap-3">
              <button @click="showFeedback = false" class="px-4 py-2 text-gray-600 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-lg">Cancel</button>
              <button class="px-4 py-2 bg-primary hover:bg-primary-hover text-white rounded-lg font-medium">Send Feedback</button>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>
    
    <!-- What's New Modal -->
    <Teleport to="body">
      <Transition name="fade">
        <div v-if="showWhatsNew" class="whats-new fixed inset-0 bg-black/50 flex items-center justify-center z-[200] p-8" @click.self="showWhatsNew = false">
          <div class="bg-white dark:bg-surface-dark rounded-2xl w-full max-w-lg overflow-hidden shadow-2xl">
            <div class="px-5 py-4 border-b border-gray-200 dark:border-border-dark flex items-center justify-between">
              <div>
                <h3 class="text-lg font-semibold text-gray-900 dark:text-white">What's New</h3>
                <p class="text-sm text-gray-500">Version 2.1.0 • December 2024</p>
              </div>
              <button @click="showWhatsNew = false" class="p-2 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-lg">
                <span class="material-symbols-outlined text-gray-400">close</span>
              </button>
            </div>
            <div class="p-5 max-h-96 overflow-y-auto space-y-4">
              <div>
                <h4 class="text-sm font-semibold text-green-600 mb-2 flex items-center gap-2">
                  <span class="material-symbols-outlined text-lg">add_circle</span>
                  New Features
                </h4>
                <ul class="space-y-2">
                  <li class="text-sm text-gray-600 dark:text-gray-400 flex items-start gap-2">
                    <span class="text-green-500 mt-1">•</span>
                    Advanced lyrics sync editor with waveform display
                  </li>
                  <li class="text-sm text-gray-600 dark:text-gray-400 flex items-start gap-2">
                    <span class="text-green-500 mt-1">•</span>
                    Command palette for quick navigation (Ctrl+K)
                  </li>
                  <li class="text-sm text-gray-600 dark:text-gray-400 flex items-start gap-2">
                    <span class="text-green-500 mt-1">•</span>
                    Keyboard shortcuts help modal
                  </li>
                </ul>
              </div>
              <div>
                <h4 class="text-sm font-semibold text-blue-600 mb-2 flex items-center gap-2">
                  <span class="material-symbols-outlined text-lg">upgrade</span>
                  Improvements
                </h4>
                <ul class="space-y-2">
                  <li class="text-sm text-gray-600 dark:text-gray-400 flex items-start gap-2">
                    <span class="text-blue-500 mt-1">•</span>
                    Faster library loading with virtual scrolling
                  </li>
                  <li class="text-sm text-gray-600 dark:text-gray-400 flex items-start gap-2">
                    <span class="text-blue-500 mt-1">•</span>
                    Improved metadata matching accuracy
                  </li>
                </ul>
              </div>
              <div>
                <h4 class="text-sm font-semibold text-amber-600 mb-2 flex items-center gap-2">
                  <span class="material-symbols-outlined text-lg">bug_report</span>
                  Bug Fixes
                </h4>
                <ul class="space-y-2">
                  <li class="text-sm text-gray-600 dark:text-gray-400 flex items-start gap-2">
                    <span class="text-amber-500 mt-1">•</span>
                    Fixed download resume issues
                  </li>
                  <li class="text-sm text-gray-600 dark:text-gray-400 flex items-start gap-2">
                    <span class="text-amber-500 mt-1">•</span>
                    Resolved memory leak in library view
                  </li>
                </ul>
              </div>
            </div>
            <div class="px-5 py-4 border-t border-gray-200 dark:border-border-dark">
              <a href="#" class="text-sm text-primary hover:underline flex items-center gap-1">
                View full changelog
                <span class="material-symbols-outlined text-[16px]">open_in_new</span>
              </a>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'

// State
const isOpen = ref(false)
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
  { id: 1, question: 'How do I add multiple accounts for the same service?', answer: 'Go to Settings > Accounts and click "Add Account" for any service. You can have multiple accounts per service.' },
  { id: 2, question: 'What audio formats are supported?', answer: 'Syncify supports FLAC, ALAC, WAV, MP3, AAC, and OGG formats. Hi-Res audio up to 24-bit/192kHz is supported.' },
  { id: 3, question: 'How does matching work between services?', answer: 'Matching uses ISRC codes, MusicBrainz IDs, and fuzzy title/artist matching to find equivalent tracks across services.' },
  { id: 4, question: 'Can I sync favorites automatically?', answer: 'Yes! Enable "Continuous Sync" in Settings > Sync to automatically sync favorites across services.' },
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
  appVersion: '2.1.0',
  os: 'Windows 11',
  connectedServices: '3 (Spotify, Qobuz, Tidal)'
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
  if (!searchQuery.value.trim()) return text
  const regex = new RegExp(`(${searchQuery.value})`, 'gi')
  return text.replace(regex, '<mark class="bg-yellow-200 dark:bg-yellow-500/30 rounded px-0.5">$1</mark>')
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

// Global keyboard listener
function handleKeydown(event: KeyboardEvent) {
  if ((event.ctrlKey && event.key === 'h') || event.key === 'F1') {
    event.preventDefault()
    isOpen.value ? close() : open()
  }
  if (event.key === 'Escape' && isOpen.value) {
    close()
  }
}

onMounted(() => {
  document.addEventListener('keydown', handleKeydown)
})

onUnmounted(() => {
  document.removeEventListener('keydown', handleKeydown)
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
