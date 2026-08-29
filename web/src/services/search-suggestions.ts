export interface SearchSuggestion {
  query: string
  type: 'recent' | 'popular' | 'tag' | 'domain'
  count?: number
  timestamp?: number
}

export class SearchSuggestionsService {
  private recentSearches: string[] = []
  private popularQueries: Map<string, number> = new Map()
  private availableTags: string[] = []
  private availableDomains: string[] = []
  
  private readonly MAX_RECENT = 10
  private readonly STORAGE_KEY = 'torimemo-search-suggestions'

  constructor() {
    this.loadFromStorage()
  }

  // Add a search query to history
  addSearchQuery(query: string) {
    if (!query || query.trim().length < 2) return
    
    query = query.trim().toLowerCase()
    
    // Update recent searches
    this.recentSearches = this.recentSearches.filter(q => q !== query)
    this.recentSearches.unshift(query)
    this.recentSearches = this.recentSearches.slice(0, this.MAX_RECENT)
    
    // Update popularity count
    const currentCount = this.popularQueries.get(query) || 0
    this.popularQueries.set(query, currentCount + 1)
    
    this.saveToStorage()
  }

  // Get search suggestions based on input
  getSuggestions(input: string, limit: number = 8): SearchSuggestion[] {
    if (!input || input.length < 1) {
      return this.getRecentSuggestions(limit)
    }

    const normalizedInput = input.toLowerCase().trim()
    const suggestions: SearchSuggestion[] = []

    // Recent searches that match
    const matchingRecent = this.recentSearches
      .filter(query => query.includes(normalizedInput))
      .slice(0, 3)
      .map(query => ({
        query,
        type: 'recent' as const,
        timestamp: Date.now()
      }))

    // Popular queries that match
    const matchingPopular = Array.from(this.popularQueries.entries())
      .filter(([query]) => query.includes(normalizedInput))
      .sort(([, a], [, b]) => b - a)
      .slice(0, 3)
      .map(([query, count]) => ({
        query,
        type: 'popular' as const,
        count
      }))

    // Tag suggestions
    const matchingTags = this.availableTags
      .filter(tag => tag.toLowerCase().includes(normalizedInput))
      .slice(0, 3)
      .map(tag => ({
        query: `tag:${tag}`,
        type: 'tag' as const
      }))

    // Domain suggestions
    const matchingDomains = this.availableDomains
      .filter(domain => domain.toLowerCase().includes(normalizedInput))
      .slice(0, 2)
      .map(domain => ({
        query: `domain:${domain}`,
        type: 'domain' as const
      }))

    // Combine and deduplicate
    suggestions.push(...matchingRecent, ...matchingPopular, ...matchingTags, ...matchingDomains)
    
    // Remove duplicates and limit
    const seen = new Set<string>()
    return suggestions
      .filter(s => {
        if (seen.has(s.query)) return false
        seen.add(s.query)
        return true
      })
      .slice(0, limit)
  }

  // Get recent searches when no input
  private getRecentSuggestions(limit: number): SearchSuggestion[] {
    return this.recentSearches
      .slice(0, limit)
      .map(query => ({
        query,
        type: 'recent' as const,
        timestamp: Date.now()
      }))
  }

  // Update available tags and domains for suggestions
  updateMetadata(tags: string[], domains: string[]) {
    this.availableTags = tags.sort()
    this.availableDomains = domains.sort()
  }

  // Get popular search queries
  getPopularQueries(limit: number = 5): string[] {
    return Array.from(this.popularQueries.entries())
      .sort(([, a], [, b]) => b - a)
      .slice(0, limit)
      .map(([query]) => query)
  }

  // Clear search history
  clearHistory() {
    this.recentSearches = []
    this.popularQueries.clear()
    this.saveToStorage()
  }

  // Get suggestion icon based on type
  getSuggestionIcon(type: string): string {
    switch (type) {
      case 'recent': return '🕒'
      case 'popular': return '🔥'
      case 'tag': return '🏷️'
      case 'domain': return '🌐'
      default: return '🔍'
    }
  }

  // Get suggestion description
  getSuggestionDescription(suggestion: SearchSuggestion): string {
    switch (suggestion.type) {
      case 'recent': return 'Recent search'
      case 'popular': return `${suggestion.count} searches`
      case 'tag': return 'Search by tag'
      case 'domain': return 'Search by domain'
      default: return ''
    }
  }

  // Load from localStorage
  private loadFromStorage() {
    try {
      const stored = localStorage.getItem(this.STORAGE_KEY)
      if (stored) {
        const data = JSON.parse(stored)
        this.recentSearches = data.recentSearches || []
        this.popularQueries = new Map(data.popularQueries || [])
      }
    } catch (error) {
      console.debug('Failed to load search suggestions from storage:', error)
    }
  }

  // Save to localStorage
  private saveToStorage() {
    try {
      const data = {
        recentSearches: this.recentSearches,
        popularQueries: Array.from(this.popularQueries.entries())
      }
      localStorage.setItem(this.STORAGE_KEY, JSON.stringify(data))
    } catch (error) {
      console.debug('Failed to save search suggestions to storage:', error)
    }
  }
}

// Export singleton instance
export const searchSuggestionsService = new SearchSuggestionsService()