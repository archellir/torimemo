import { LitElement, html, css } from 'lit'
import { customElement, state, property } from 'lit/decorators.js'

export interface AdvancedSearchFilters {
  query: string
  tags: string[]
  excludeTags: string[]
  dateRange: {
    start: string | null
    end: string | null
  }
  favoritesOnly: boolean
  healthStatus: string[]
  domainFilter: string
  hasDescription: boolean | null
  sortBy: string
  sortOrder: 'asc' | 'desc'
}

@customElement('advanced-search')
export class AdvancedSearch extends LitElement {
  @property({ type: Array }) availableTags: string[] = []
  @property({ type: Array }) availableDomains: string[] = []
  @state() private _isExpanded = false
  @state() private _filters: AdvancedSearchFilters = {
    query: '',
    tags: [],
    excludeTags: [],
    dateRange: { start: null, end: null },
    favoritesOnly: false,
    healthStatus: [],
    domainFilter: '',
    hasDescription: null,
    sortBy: 'created_at',
    sortOrder: 'desc'
  }
  @state() private _activeFiltersCount = 0

  static styles = css`
    :host {
      display: block;
      background: var(--bg-card);
      border: 1px solid var(--border-color);
      border-radius: 0.75rem;
      margin-bottom: 1rem;
      backdrop-filter: blur(10px);
      box-shadow: var(--shadow-sm);
      overflow: hidden;
      transition: all 0.3s ease;
    }

    .search-header {
      display: flex;
      align-items: center;
      justify-content: space-between;
      padding: 1rem;
      background: var(--bg-secondary);
      border-bottom: 1px solid var(--border-color);
      cursor: pointer;
      transition: background-color 0.3s ease;
    }

    .search-header:hover {
      background: var(--bg-card-hover);
    }

    .search-title {
      display: flex;
      align-items: center;
      gap: 0.75rem;
      font-weight: bold;
      color: var(--text-primary);
    }

    .filter-count {
      background: var(--accent-primary);
      color: var(--bg-primary);
      padding: 0.2rem 0.5rem;
      border-radius: 50%;
      font-size: 0.75rem;
      font-weight: bold;
      min-width: 20px;
      text-align: center;
    }

    .expand-icon {
      font-size: 1.2rem;
      transition: transform 0.3s ease;
      color: var(--text-muted);
    }

    .expanded .expand-icon {
      transform: rotate(180deg);
    }

    .search-content {
      max-height: 0;
      overflow: hidden;
      transition: max-height 0.3s ease;
    }

    .expanded .search-content {
      max-height: 2000px;
    }

    .search-grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
      gap: 1.5rem;
      padding: 1.5rem;
    }

    .filter-group {
      display: flex;
      flex-direction: column;
      gap: 0.75rem;
    }

    .filter-label {
      font-weight: 600;
      color: var(--text-primary);
      font-size: 0.9rem;
      display: flex;
      align-items: center;
      gap: 0.5rem;
    }

    .filter-input {
      background: var(--bg-primary);
      border: 1px solid var(--border-color);
      color: var(--text-primary);
      padding: 0.75rem;
      border-radius: 0.5rem;
      font-size: 0.9rem;
      transition: border-color 0.3s ease;
    }

    .filter-input:focus {
      outline: none;
      border-color: var(--accent-primary);
      box-shadow: 0 0 0 2px rgba(var(--accent-primary), 0.2);
    }

    .tag-selector {
      position: relative;
    }

    .tag-input {
      width: 100%;
      padding-right: 2.5rem;
    }

    .tag-dropdown {
      position: absolute;
      top: 100%;
      left: 0;
      right: 0;
      background: var(--bg-card);
      border: 1px solid var(--border-color);
      border-radius: 0.5rem;
      max-height: 200px;
      overflow-y: auto;
      z-index: 1000;
      box-shadow: var(--shadow-lg);
    }

    .tag-option {
      padding: 0.75rem;
      cursor: pointer;
      transition: background-color 0.2s ease;
      display: flex;
      align-items: center;
      justify-content: space-between;
    }

    .tag-option:hover {
      background: var(--bg-card-hover);
    }

    .tag-option.selected {
      background: rgba(var(--accent-primary), 0.1);
      color: var(--accent-primary);
    }

    .selected-tags {
      display: flex;
      flex-wrap: wrap;
      gap: 0.5rem;
      margin-top: 0.5rem;
    }

    .tag-chip {
      background: var(--accent-secondary);
      color: var(--bg-primary);
      padding: 0.25rem 0.75rem;
      border-radius: 1rem;
      font-size: 0.8rem;
      display: flex;
      align-items: center;
      gap: 0.5rem;
      transition: background-color 0.3s ease;
    }

    .tag-chip.exclude {
      background: var(--accent-danger);
    }

    .tag-chip-remove {
      background: none;
      border: none;
      color: inherit;
      cursor: pointer;
      font-size: 1rem;
      line-height: 1;
      padding: 0;
      transition: opacity 0.3s ease;
    }

    .tag-chip-remove:hover {
      opacity: 0.7;
    }

    .checkbox-group {
      display: flex;
      flex-direction: column;
      gap: 0.5rem;
    }

    .checkbox-option {
      display: flex;
      align-items: center;
      gap: 0.75rem;
      cursor: pointer;
      transition: color 0.3s ease;
    }

    .checkbox-option:hover {
      color: var(--accent-primary);
    }

    .checkbox {
      width: 18px;
      height: 18px;
      border: 2px solid var(--border-color);
      border-radius: 3px;
      transition: all 0.3s ease;
      accent-color: var(--accent-primary);
    }

    .date-inputs {
      display: flex;
      gap: 0.75rem;
      align-items: center;
    }

    .date-input {
      flex: 1;
    }

    .date-separator {
      color: var(--text-muted);
      font-weight: bold;
    }

    .sort-group {
      display: flex;
      gap: 0.75rem;
    }

    .sort-select {
      flex: 1;
    }

    .actions {
      display: flex;
      gap: 1rem;
      padding: 1rem 1.5rem;
      background: var(--bg-secondary);
      border-top: 1px solid var(--border-color);
    }

    .action-button {
      background: var(--bg-primary);
      border: 1px solid var(--border-color);
      color: var(--text-primary);
      padding: 0.75rem 1.5rem;
      border-radius: 0.5rem;
      cursor: pointer;
      transition: all 0.3s ease;
      font-weight: 500;
    }

    .action-button:hover {
      border-color: var(--accent-primary);
      background: rgba(var(--accent-primary), 0.1);
      color: var(--accent-primary);
    }

    .action-button.primary {
      background: var(--accent-primary);
      color: var(--bg-primary);
      border-color: var(--accent-primary);
    }

    .action-button.primary:hover {
      background: var(--accent-secondary);
      border-color: var(--accent-secondary);
    }

    .presets {
      display: flex;
      gap: 0.75rem;
      flex-wrap: wrap;
    }

    .preset-button {
      background: var(--bg-secondary);
      border: 1px solid var(--border-color);
      color: var(--text-secondary);
      padding: 0.5rem 1rem;
      border-radius: 0.5rem;
      cursor: pointer;
      transition: all 0.3s ease;
      font-size: 0.85rem;
    }

    .preset-button:hover {
      border-color: var(--accent-secondary);
      background: rgba(var(--accent-secondary), 0.1);
      color: var(--accent-secondary);
    }

    @media (max-width: 768px) {
      .search-grid {
        grid-template-columns: 1fr;
        gap: 1rem;
        padding: 1rem;
      }
      
      .actions {
        flex-direction: column;
        gap: 0.75rem;
      }
      
      .presets {
        flex-direction: column;
      }
    }
  `

  connectedCallback() {
    super.connectedCallback()
    this._updateActiveFiltersCount()
  }

  render() {
    return html`
      <div class="search-header ${this._isExpanded ? 'expanded' : ''}" @click=${this._toggleExpanded}>
        <div class="search-title">
          🔍 Advanced Search
          ${this._activeFiltersCount > 0 ? html`
            <span class="filter-count">${this._activeFiltersCount}</span>
          ` : ''}
        </div>
        <span class="expand-icon">▼</span>
      </div>

      <div class="search-content">
        <div class="search-grid">
          <!-- Text Search -->
          <div class="filter-group">
            <label class="filter-label">
              📝 Text Search
            </label>
            <input
              type="text"
              class="filter-input"
              placeholder="Search title, description, URL..."
              .value=${this._filters.query}
              @input=${this._handleQueryChange}>
          </div>

          <!-- Include Tags -->
          <div class="filter-group">
            <label class="filter-label">
              🏷️ Include Tags
            </label>
            ${this._renderTagSelector('include')}
          </div>

          <!-- Exclude Tags -->
          <div class="filter-group">
            <label class="filter-label">
              🚫 Exclude Tags
            </label>
            ${this._renderTagSelector('exclude')}
          </div>

          <!-- Date Range -->
          <div class="filter-group">
            <label class="filter-label">
              📅 Date Added
            </label>
            <div class="date-inputs">
              <input
                type="date"
                class="filter-input date-input"
                .value=${this._filters.dateRange.start || ''}
                @change=${this._handleStartDateChange}>
              <span class="date-separator">—</span>
              <input
                type="date"
                class="filter-input date-input"
                .value=${this._filters.dateRange.end || ''}
                @change=${this._handleEndDateChange}>
            </div>
          </div>

          <!-- Health Status -->
          <div class="filter-group">
            <label class="filter-label">
              ❤️ Link Health
            </label>
            <div class="checkbox-group">
              ${['healthy', 'broken', 'slow', 'redirect'].map(status => html`
                <label class="checkbox-option">
                  <input
                    type="checkbox"
                    class="checkbox"
                    .checked=${this._filters.healthStatus.includes(status)}
                    @change=${(e: Event) => this._toggleHealthStatus(status, (e.target as HTMLInputElement).checked)}>
                  ${this._getHealthStatusLabel(status)}
                </label>
              `)}
            </div>
          </div>

          <!-- Domain Filter -->
          <div class="filter-group">
            <label class="filter-label">
              🌐 Domain
            </label>
            <input
              type="text"
              class="filter-input"
              placeholder="example.com"
              .value=${this._filters.domainFilter}
              @input=${this._handleDomainChange}
              list="domain-suggestions">
            <datalist id="domain-suggestions">
              ${this.availableDomains.map(domain => html`<option value="${domain}">`)}
            </datalist>
          </div>

          <!-- Additional Filters -->
          <div class="filter-group">
            <label class="filter-label">
              ⚙️ Additional Filters
            </label>
            <div class="checkbox-group">
              <label class="checkbox-option">
                <input
                  type="checkbox"
                  class="checkbox"
                  .checked=${this._filters.favoritesOnly}
                  @change=${this._handleFavoritesChange}>
                ⭐ Favorites only
              </label>
              <label class="checkbox-option">
                <input
                  type="checkbox"
                  class="checkbox"
                  .checked=${this._filters.hasDescription === true}
                  @change=${this._handleDescriptionChange}>
                📄 Has description
              </label>
            </div>
          </div>

          <!-- Sort Options -->
          <div class="filter-group">
            <label class="filter-label">
              🔤 Sort By
            </label>
            <div class="sort-group">
              <select class="filter-input sort-select" .value=${this._filters.sortBy} @change=${this._handleSortByChange}>
                <option value="created_at">Date Added</option>
                <option value="updated_at">Date Modified</option>
                <option value="title">Title</option>
                <option value="url">URL</option>
              </select>
              <select class="filter-input sort-select" .value=${this._filters.sortOrder} @change=${this._handleSortOrderChange}>
                <option value="desc">Newest First</option>
                <option value="asc">Oldest First</option>
              </select>
            </div>
          </div>
        </div>

        <div class="actions">
          <div class="presets">
            <button class="preset-button" @click=${this._applyPreset.bind(this, 'recent')}>
              📅 Recent (7 days)
            </button>
            <button class="preset-button" @click=${this._applyPreset.bind(this, 'favorites')}>
              ⭐ Favorites
            </button>
            <button class="preset-button" @click=${this._applyPreset.bind(this, 'untagged')}>
              🏷️ Untagged
            </button>
            <button class="preset-button" @click=${this._applyPreset.bind(this, 'broken')}>
              ❌ Broken Links
            </button>
          </div>
          
          <div style="margin-left: auto; display: flex; gap: 1rem;">
            <button class="action-button" @click=${this._clearFilters}>
              Clear All
            </button>
            <button class="action-button primary" @click=${this._applyFilters}>
              Apply Filters
            </button>
          </div>
        </div>
      </div>
    `
  }

  private _renderTagSelector(type: 'include' | 'exclude') {
    const selectedTags = type === 'include' ? this._filters.tags : this._filters.excludeTags
    
    return html`
      <div class="tag-selector">
        <input
          type="text"
          class="filter-input tag-input"
          placeholder="Type to search tags..."
          @input=${(e: Event) => this._handleTagInput(e)}>
        ${this._renderTagDropdown()}
        ${selectedTags.length > 0 ? html`
          <div class="selected-tags">
            ${selectedTags.map(tag => html`
              <span class="tag-chip ${type === 'exclude' ? 'exclude' : ''}">
                ${tag}
                <button 
                  class="tag-chip-remove" 
                  @click=${() => this._removeTag(tag, type)}>
                  ✕
                </button>
              </span>
            `)}
          </div>
        ` : ''}
      </div>
    `
  }

  private _renderTagDropdown() {
    // This would be expanded based on input state
    return html``
  }

  private _toggleExpanded() {
    this._isExpanded = !this._isExpanded
  }

  private _handleQueryChange(e: Event) {
    const input = e.target as HTMLInputElement
    this._filters = { ...this._filters, query: input.value }
    this._updateActiveFiltersCount()
  }

  private _handleTagInput(_e: Event) {
    // Handle tag autocomplete - to be implemented
  }

  private _removeTag(tag: string, type: 'include' | 'exclude') {
    if (type === 'include') {
      this._filters.tags = this._filters.tags.filter(t => t !== tag)
    } else {
      this._filters.excludeTags = this._filters.excludeTags.filter(t => t !== tag)
    }
    this._updateActiveFiltersCount()
    this.requestUpdate()
  }

  private _handleStartDateChange(e: Event) {
    const input = e.target as HTMLInputElement
    this._filters = {
      ...this._filters,
      dateRange: { ...this._filters.dateRange, start: input.value || null }
    }
    this._updateActiveFiltersCount()
  }

  private _handleEndDateChange(e: Event) {
    const input = e.target as HTMLInputElement
    this._filters = {
      ...this._filters,
      dateRange: { ...this._filters.dateRange, end: input.value || null }
    }
    this._updateActiveFiltersCount()
  }

  private _toggleHealthStatus(status: string, checked: boolean) {
    if (checked) {
      this._filters.healthStatus = [...this._filters.healthStatus, status]
    } else {
      this._filters.healthStatus = this._filters.healthStatus.filter(s => s !== status)
    }
    this._updateActiveFiltersCount()
    this.requestUpdate()
  }

  private _handleDomainChange(e: Event) {
    const input = e.target as HTMLInputElement
    this._filters = { ...this._filters, domainFilter: input.value }
    this._updateActiveFiltersCount()
  }

  private _handleFavoritesChange(e: Event) {
    const checkbox = e.target as HTMLInputElement
    this._filters = { ...this._filters, favoritesOnly: checkbox.checked }
    this._updateActiveFiltersCount()
  }

  private _handleDescriptionChange(e: Event) {
    const checkbox = e.target as HTMLInputElement
    this._filters = { ...this._filters, hasDescription: checkbox.checked ? true : null }
    this._updateActiveFiltersCount()
  }

  private _handleSortByChange(e: Event) {
    const select = e.target as HTMLSelectElement
    this._filters = { ...this._filters, sortBy: select.value }
    this._updateActiveFiltersCount()
  }

  private _handleSortOrderChange(e: Event) {
    const select = e.target as HTMLSelectElement
    this._filters = { ...this._filters, sortOrder: select.value as 'asc' | 'desc' }
    this._updateActiveFiltersCount()
  }

  private _getHealthStatusLabel(status: string): string {
    switch (status) {
      case 'healthy': return '✅ Healthy'
      case 'broken': return '❌ Broken'
      case 'slow': return '⚠️ Slow'
      case 'redirect': return '🔄 Redirects'
      default: return status
    }
  }

  private _applyPreset(preset: string) {
    const now = new Date()
    const week = new Date(now.getTime() - 7 * 24 * 60 * 60 * 1000)

    switch (preset) {
      case 'recent':
        this._filters = {
          ...this._filters,
          dateRange: { start: week.toISOString().split('T')[0], end: null }
        }
        break
      case 'favorites':
        this._filters = { ...this._filters, favoritesOnly: true }
        break
      case 'untagged':
        this._filters = { ...this._filters, tags: [], excludeTags: [] }
        break
      case 'broken':
        this._filters = { ...this._filters, healthStatus: ['broken'] }
        break
    }
    this._updateActiveFiltersCount()
    this.requestUpdate()
  }

  private _clearFilters() {
    this._filters = {
      query: '',
      tags: [],
      excludeTags: [],
      dateRange: { start: null, end: null },
      favoritesOnly: false,
      healthStatus: [],
      domainFilter: '',
      hasDescription: null,
      sortBy: 'created_at',
      sortOrder: 'desc'
    }
    this._updateActiveFiltersCount()
    this.requestUpdate()
  }

  private _applyFilters() {
    this.dispatchEvent(new CustomEvent('filters-changed', {
      detail: { filters: this._filters }
    }))
  }

  private _updateActiveFiltersCount() {
    let count = 0
    if (this._filters.query) count++
    if (this._filters.tags.length > 0) count++
    if (this._filters.excludeTags.length > 0) count++
    if (this._filters.dateRange.start || this._filters.dateRange.end) count++
    if (this._filters.favoritesOnly) count++
    if (this._filters.healthStatus.length > 0) count++
    if (this._filters.domainFilter) count++
    if (this._filters.hasDescription !== null) count++
    if (this._filters.sortBy !== 'created_at' || this._filters.sortOrder !== 'desc') count++
    
    this._activeFiltersCount = count
    this.requestUpdate()
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'advanced-search': AdvancedSearch
  }
}