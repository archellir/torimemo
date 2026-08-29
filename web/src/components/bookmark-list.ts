import { LitElement, html, css } from 'lit'
import { customElement, state, property } from 'lit/decorators.js'
import { apiService, type Bookmark, type BookmarkListResponse } from '../services/api.ts'
import './bookmark-item.ts'
import './bulk-actions.ts'

@customElement('bookmark-list')
export class BookmarkList extends LitElement {
  @state() private _bookmarks: Bookmark[] = []
  @state() private _loading = true
  @state() private _error: string | null = null
  @state() private _stats = {
    total: 0,
    favorites: 0,
    tags: 0,
    hasMore: false,
    page: 1
  }
  @state() private _selectedIndex = -1
  @state() private _selectedBookmarks = new Set<number>()
  @state() private _selectionMode = false
  @state() private _availableTags: string[] = []
  @state() private _searchResults = new Map<number, any>()

  @property() searchQuery = ''
  @property() tagFilter = ''
  @property() favoritesOnly = false
  @property() advancedFilters: any = null

  static styles = css`
    :host {
      display: block;
    }

    .loading {
      text-align: center;
      padding: 2rem;
      color: var(--accent-primary);
      display: flex;
      flex-direction: column;
      align-items: center;
      gap: 1rem;
    }

    .loading-spinner {
      width: 40px;
      height: 40px;
      border: 3px solid rgba(var(--accent-primary), 0.2);
      border-top: 3px solid var(--accent-primary);
      border-radius: 50%;
      animation: spin 1s linear infinite;
    }

    @keyframes spin {
      0% { transform: rotate(0deg); }
      100% { transform: rotate(360deg); }
    }

    .error {
      text-align: center;
      padding: 2rem;
      color: var(--accent-danger);
      background: rgba(var(--accent-danger), 0.1);
      border: 1px solid rgba(var(--accent-danger), 0.3);
      border-radius: 0.5rem;
      margin-bottom: 1rem;
    }

    .retry-button {
      background: transparent;
      border: 1px solid var(--accent-danger);
      color: var(--accent-danger);
      padding: 0.5rem 1rem;
      border-radius: 0.25rem;
      cursor: pointer;
      margin-top: 0.5rem;
      transition: all 0.3s ease;
    }

    .retry-button:hover {
      background: var(--accent-danger);
      color: var(--bg-primary);
    }

    .empty-state {
      text-align: center;
      padding: 3rem 1rem;
      color: var(--text-muted);
    }

    .empty-icon {
      font-size: 3rem;
      margin-bottom: 1rem;
      opacity: 0.5;
    }

    .bookmark-grid {
      display: grid;
      gap: 1rem;
      grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
    }

    bookmark-item {
      transition: transform 0.2s ease, box-shadow 0.2s ease;
    }

    bookmark-item.selected {
      transform: scale(1.02);
      box-shadow: 0 0 0 2px var(--accent-primary);
      border-radius: 0.75rem;
    }

    :host(:focus) {
      outline: none;
    }

    .stats {
      display: flex;
      justify-content: space-between;
      align-items: center;
      margin-bottom: 1.5rem;
      padding: 1rem;
      background: var(--bg-card);
      border: 1px solid var(--border-color);
      border-radius: 0.5rem;
    }

    .stats-item {
      display: flex;
      flex-direction: column;
      align-items: center;
      gap: 0.25rem;
    }

    .stats-number {
      font-size: 1.25rem;
      font-weight: bold;
      color: var(--accent-primary);
    }

    .stats-label {
      font-size: 0.75rem;
      color: var(--text-secondary);
      text-transform: uppercase;
      letter-spacing: 1px;
    }

    .bulk-select-button {
      background: var(--bg-secondary);
      border: 1px solid var(--border-color);
      color: var(--text-primary);
      padding: 0.5rem 1rem;
      border-radius: 0.25rem;
      cursor: pointer;
      transition: all 0.3s ease;
      font-size: 0.8rem;
      text-transform: uppercase;
      letter-spacing: 0.5px;
    }

    .bulk-select-button:hover {
      border-color: var(--accent-primary);
      background: rgba(var(--accent-primary), 0.1);
      color: var(--accent-primary);
    }

    .bulk-select-button.active {
      background: var(--accent-primary);
      color: var(--bg-primary);
      border-color: var(--accent-primary);
    }

    .load-more {
      text-align: center;
      margin-top: 2rem;
    }

    .load-more-button {
      background: var(--bg-card);
      border: 1px solid var(--accent-secondary);
      color: var(--accent-secondary);
      padding: 0.75rem 1.5rem;
      border-radius: 0.5rem;
      cursor: pointer;
      transition: all 0.3s ease;
      position: relative;
      overflow: hidden;
    }

    .load-more-button:hover {
      background: var(--accent-secondary);
      color: var(--bg-primary);
      box-shadow: var(--shadow-md);
    }

    .load-more-button:disabled {
      opacity: 0.5;
      cursor: not-allowed;
    }

    /* Mobile responsiveness */
    @media (max-width: 1024px) {
      .bookmark-grid {
        grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
        gap: 0.75rem;
      }
    }

    @media (max-width: 768px) {
      .bookmark-grid {
        grid-template-columns: 1fr;
        gap: 0.75rem;
      }
      
      .stats {
        flex-direction: row;
        flex-wrap: wrap;
        gap: 0.75rem;
        padding: 0.75rem;
        text-align: center;
      }
      
      .stats-item {
        min-width: 80px;
        flex: 1;
      }
      
      .stats-number {
        font-size: 1.125rem;
      }
      
      .stats-label {
        font-size: 0.7rem;
      }
      
      .bulk-select-button {
        padding: 0.375rem 0.75rem;
        font-size: 0.75rem;
      }
      
      .load-more-button {
        padding: 0.625rem 1.25rem;
        font-size: 0.9rem;
        width: 100%;
        max-width: 280px;
      }
    }

    @media (max-width: 480px) {
      .bookmark-grid {
        gap: 0.5rem;
      }
      
      .stats {
        padding: 0.5rem;
        gap: 0.5rem;
      }
      
      .stats-item {
        min-width: 70px;
      }
      
      .stats-number {
        font-size: 1rem;
      }
      
      .stats-label {
        font-size: 0.65rem;
      }
      
      .empty-state {
        padding: 2rem 0.5rem;
      }
      
      .empty-icon {
        font-size: 2.5rem;
      }
      
      .loading {
        padding: 1.5rem 0.5rem;
      }
      
      .error {
        padding: 1.5rem 0.75rem;
        margin: 0.5rem;
      }
    }
  `

  connectedCallback() {
    super.connectedCallback()
    this.loadBookmarks()
    this._loadAvailableTags()
    this._setupKeyboardNavigation()
  }

  disconnectedCallback() {
    super.disconnectedCallback()
    this.removeEventListener('keydown', this._handleKeydown)
  }

  private _setupKeyboardNavigation() {
    this.addEventListener('keydown', this._handleKeydown)
    this.tabIndex = 0 // Make the component focusable
  }

  private _handleKeydown = (e: KeyboardEvent) => {
    if (this._bookmarks.length === 0) return

    switch (e.key) {
      case 'ArrowDown':
      case 'j':
        e.preventDefault()
        this._selectedIndex = Math.min(this._selectedIndex + 1, this._bookmarks.length - 1)
        this._scrollToSelected()
        break
      case 'ArrowUp':
      case 'k':
        e.preventDefault()
        this._selectedIndex = Math.max(this._selectedIndex - 1, 0)
        this._scrollToSelected()
        break
      case 'Enter':
        e.preventDefault()
        if (this._selectedIndex >= 0) {
          const bookmark = this._bookmarks[this._selectedIndex]
          window.open(bookmark.url, '_blank')
        }
        break
      case 'f':
        e.preventDefault()
        if (this._selectedIndex >= 0) {
          this._toggleFavorite(this._bookmarks[this._selectedIndex].id)
        }
        break
      case 'e':
        e.preventDefault()
        if (this._selectedIndex >= 0) {
          this._editBookmark(this._bookmarks[this._selectedIndex])
        }
        break
      case 'd':
        e.preventDefault()
        if (this._selectedIndex >= 0) {
          this._deleteBookmark(this._bookmarks[this._selectedIndex].id)
        }
        break
    }
  }

  private _scrollToSelected() {
    const bookmarkItems = this.shadowRoot?.querySelectorAll('bookmark-item')
    if (bookmarkItems && this._selectedIndex >= 0) {
      bookmarkItems[this._selectedIndex]?.scrollIntoView({ 
        behavior: 'smooth', 
        block: 'center' 
      })
    }
  }

  private async _toggleFavorite(bookmarkId: number) {
    const bookmark = this._bookmarks.find(b => b.id === bookmarkId)
    if (!bookmark) return

    try {
      const updatedBookmark = await apiService.updateBookmark(bookmarkId, {
        is_favorite: !bookmark.is_favorite
      })

      this._bookmarks = this._bookmarks.map(b => 
        b.id === bookmarkId ? updatedBookmark : b
      )

      if (updatedBookmark.is_favorite) {
        this._stats.favorites++
      } else {
        this._stats.favorites--
      }
    } catch (error) {
      console.error('Failed to toggle favorite:', error)
    }
  }

  private _editBookmark(bookmark: Bookmark) {
    this.dispatchEvent(new CustomEvent('edit-bookmark', {
      detail: { bookmark },
      bubbles: true
    }))
  }

  private async _deleteBookmark(bookmarkId: number) {
    const bookmark = this._bookmarks.find(b => b.id === bookmarkId)
    if (!bookmark) return
    
    if (!confirm(`Delete "${bookmark.title}"?`)) return

    try {
      await apiService.deleteBookmark(bookmarkId)
      
      this._bookmarks = this._bookmarks.filter(b => b.id !== bookmarkId)
      
      this._stats.total--
      if (bookmark.is_favorite) {
        this._stats.favorites--
      }

      // Adjust selected index if needed
      if (this._selectedIndex >= this._bookmarks.length) {
        this._selectedIndex = this._bookmarks.length - 1
      }

      this.dispatchEvent(new CustomEvent('bookmark-deleted', {
        bubbles: true,
        detail: { bookmark }
      }))
    } catch (error) {
      console.error('Failed to delete bookmark:', error)
    }
  }

  updated(changedProperties: Map<string, any>) {
    if (changedProperties.has('searchQuery') || 
        changedProperties.has('tagFilter') || 
        changedProperties.has('favoritesOnly') ||
        changedProperties.has('advancedFilters')) {
      this.loadBookmarks(true) // Reset to first page
    }
  }

  async loadBookmarks(reset = false) {
    if (reset) {
      this._stats.page = 1
      this._bookmarks = []
    }

    this._loading = true
    this._error = null

    try {
      let response: BookmarkListResponse

      // Use advanced search if filters are present
      if (this.advancedFilters) {
        const searchResponse = await this._performAdvancedSearch()
        response = searchResponse
      } else if (this.searchQuery && this.searchQuery.trim()) {
        // Use full-text search if search query exists
        const searchResponse = await apiService.searchBookmarks(this.searchQuery, 20)
        // Store search results with snippets for highlighting
        this._searchResults = new Map(searchResponse.results.map(r => [r.id, r]))
        // Convert SearchResult[] to BookmarkListResponse format
        response = {
          bookmarks: searchResponse.results,
          total: searchResponse.count,
          page: 1,
          limit: 20,
          has_more: false,
          total_pages: 1,
          tag_count: 0,
          favorite_count: searchResponse.results.filter(b => b.is_favorite).length
        }
      } else {
        // Clear search results when not searching
        this._searchResults.clear()
        response = await apiService.getBookmarks({
          page: this._stats.page,
          limit: 20,
          tag: this.tagFilter || undefined,
          favorites: this.favoritesOnly || undefined
        })
      }

      if (reset) {
        this._bookmarks = response.bookmarks
      } else {
        this._bookmarks = [...this._bookmarks, ...response.bookmarks]
      }

      this._stats = {
        total: response.total,
        favorites: response.favorite_count,
        tags: response.tag_count,
        hasMore: response.has_more,
        page: response.page
      }
    } catch (error) {
      this._error = error instanceof Error ? error.message : 'Failed to load bookmarks'
    } finally {
      this._loading = false
    }
  }

  async loadMore() {
    if (this._loading || !this._stats.hasMore) return
    
    this._stats.page++
    await this.loadBookmarks()
  }

  render() {
    if (this._loading && this._bookmarks.length === 0) {
      return html`
        <div class="loading">
          <div class="loading-spinner"></div>
          <div class="neon-cyan">Loading bookmarks...</div>
        </div>
      `
    }

    if (this._error) {
      return html`
        <div class="error">
          <div>⚠️ ${this._error}</div>
          <button class="retry-button" @click=${() => this.loadBookmarks(true)}>
            Retry
          </button>
        </div>
      `
    }

    if (this._bookmarks.length === 0) {
      return html`
        <div class="empty-state">
          <div class="empty-icon">${this.searchQuery ? '🔍' : '🔖'}</div>
          <h3>${this.searchQuery ? 'No search results' : 'No bookmarks found'}</h3>
          ${this.searchQuery ? html`
            <p>No results for <strong>"${this.searchQuery}"</strong></p>
            <p>Try different keywords or check your spelling.</p>
          ` : this.tagFilter ? html`
            <p>No bookmarks found with the tag <strong>"${this.tagFilter}"</strong></p>
          ` : this.favoritesOnly ? html`
            <p>No favorite bookmarks yet. Star some bookmarks to see them here!</p>
          ` : html`
            <p>Add your first bookmark to get started!</p>
          `}
        </div>
      `
    }

    return html`
      <div class="stats">
        <div class="stats-item">
          <div class="stats-number">${this._stats.total}</div>
          <div class="stats-label">Total</div>
        </div>
        <div class="stats-item">
          <div class="stats-number">${this._stats.favorites}</div>
          <div class="stats-label">Favorites</div>
        </div>
        <div class="stats-item">
          <div class="stats-number">${this._stats.tags}</div>
          <div class="stats-label">Tags</div>
        </div>
        <div class="stats-item">
          <button class="bulk-select-button" @click=${this._toggleSelectionMode}>
            ${this._selectionMode ? '✓ Exit Selection' : '☑️ Select Multiple'}
          </button>
        </div>
      </div>

      ${this._selectedBookmarks.size > 0 ? html`
        <bulk-actions
          .selectedBookmarks=${this._getSelectedBookmarks()}
          .availableTags=${this._availableTags}
          @clear-selection=${this._clearSelection}
          @bulk-action-complete=${this._handleBulkActionComplete}>
        </bulk-actions>
      ` : ''}

      <div class="bookmark-grid">
        ${this._bookmarks.map((bookmark, index) => html`
          <bookmark-item 
            .bookmark=${bookmark}
            .isSelected=${this._selectedBookmarks.has(bookmark.id)}
            .selectionMode=${this._selectionMode}
            .searchSnippet=${this._searchResults.get(bookmark.id)?.snippet || ''}
            class=${index === this._selectedIndex ? 'selected' : ''}
            @selection-toggle=${this._handleSelectionToggle}
            @toggle-favorite=${this._handleToggleFavorite}
            @delete=${this._handleDelete}
            @edit=${this._handleEdit}>
          </bookmark-item>
        `)}
      </div>

      ${this._stats.hasMore && !this.searchQuery ? html`
        <div class="load-more">
          <button 
            class="load-more-button"
            ?disabled=${this._loading}
            @click=${this.loadMore}>
            ${this._loading ? 'Loading...' : 'Load More'}
          </button>
        </div>
      ` : ''}
    `
  }

  private async _handleToggleFavorite(e: CustomEvent) {
    const bookmarkId = e.detail.id
    const bookmark = this._bookmarks.find(b => b.id === bookmarkId)
    if (!bookmark) return

    try {
      const updatedBookmark = await apiService.updateBookmark(bookmarkId, {
        is_favorite: !bookmark.is_favorite
      })

      // Update local state
      this._bookmarks = this._bookmarks.map(b => 
        b.id === bookmarkId ? updatedBookmark : b
      )

      // Update stats
      if (updatedBookmark.is_favorite) {
        this._stats.favorites++
      } else {
        this._stats.favorites--
      }
    } catch (error) {
      console.error('Failed to toggle favorite:', error)
      // TODO: Show toast notification
    }
  }

  private async _handleDelete(e: CustomEvent) {
    const bookmarkId = e.detail.id
    
    try {
      await apiService.deleteBookmark(bookmarkId)
      
      // Remove from local state
      const bookmark = this._bookmarks.find(b => b.id === bookmarkId)
      this._bookmarks = this._bookmarks.filter(b => b.id !== bookmarkId)
      
      // Update stats
      this._stats.total--
      if (bookmark?.is_favorite) {
        this._stats.favorites--
      }

      // Notify parent to refresh tag cloud
      this.dispatchEvent(new CustomEvent('bookmark-deleted', {
        bubbles: true,
        detail: { bookmark }
      }))
    } catch (error) {
      console.error('Failed to delete bookmark:', error)
      // TODO: Show toast notification
    }
  }

  private _handleEdit(e: CustomEvent) {
    // Dispatch event to parent component to open edit dialog
    this.dispatchEvent(new CustomEvent('edit-bookmark', {
      detail: e.detail,
      bubbles: true
    }))
  }

  // Bulk operations methods
  private _toggleSelectionMode() {
    this._selectionMode = !this._selectionMode
    if (!this._selectionMode) {
      this._selectedBookmarks.clear()
    }
  }

  private _handleSelectionToggle(e: CustomEvent) {
    const { bookmark, selected } = e.detail
    if (selected) {
      this._selectedBookmarks.add(bookmark.id)
    } else {
      this._selectedBookmarks.delete(bookmark.id)
    }
    this.requestUpdate()
  }

  private _clearSelection() {
    this._selectedBookmarks.clear()
    this._selectionMode = false
    this.requestUpdate()
  }

  private _getSelectedBookmarks(): Bookmark[] {
    return this._bookmarks.filter(bookmark => this._selectedBookmarks.has(bookmark.id))
  }

  private _handleBulkActionComplete(e: CustomEvent) {
    const { success, message } = e.detail
    
    // Refresh bookmarks after bulk action
    this.loadBookmarks(true)
    
    // Dispatch event to parent for notification
    this.dispatchEvent(new CustomEvent('bulk-action-result', {
      detail: { success, message }
    }))
  }

  // Load available tags for bulk actions
  private async _loadAvailableTags() {
    try {
      const response = await fetch('/api/tags')
      if (response.ok) {
        const tags = await response.json()
        this._availableTags = tags.map((tag: any) => tag.name)
      }
    } catch (error) {
      console.error('Failed to load tags:', error)
    }
  }

  // Perform advanced search using backend API
  private async _performAdvancedSearch(): Promise<BookmarkListResponse> {
    const searchRequest = {
      query: this.advancedFilters.query || '',
      tags: this.advancedFilters.tags || [],
      exclude_tags: this.advancedFilters.excludeTags || [],
      domain: this.advancedFilters.domainFilter || '',
      favorites_only: this.advancedFilters.favoritesOnly || false,
      date_from: this.advancedFilters.dateRange?.start || null,
      date_to: this.advancedFilters.dateRange?.end || null,
      sort_by: this.advancedFilters.sortBy || 'created_at',
      sort_order: this.advancedFilters.sortOrder || 'desc',
      page: this._stats.page,
      limit: 20
    }

    const response = await fetch('/api/search/advanced', {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify(searchRequest)
    })

    if (!response.ok) {
      throw new Error(`Advanced search failed: ${response.statusText}`)
    }

    const data = await response.json()
    
    // Convert to BookmarkListResponse format
    return {
      bookmarks: data.bookmarks,
      total: data.total,
      page: data.page,
      limit: data.limit,
      has_more: data.has_more,
      total_pages: Math.ceil(data.total / data.limit),
      tag_count: 0, // Advanced search doesn't provide tag count
      favorite_count: data.bookmarks.filter((b: any) => b.is_favorite).length
    }
  }
}