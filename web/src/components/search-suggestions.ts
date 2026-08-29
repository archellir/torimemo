import { LitElement, html, css } from 'lit'
import { customElement, property, state } from 'lit/decorators.js'
import { searchSuggestionsService, type SearchSuggestion } from '../services/search-suggestions.ts'

@customElement('search-suggestions')
export class SearchSuggestions extends LitElement {
  @property({ type: String }) query = ''
  @property({ type: Boolean }) visible = false
  @property({ type: Number }) selectedIndex = -1
  @state() private _suggestions: SearchSuggestion[] = []

  static styles = css`
    :host {
      position: absolute;
      top: 100%;
      left: 0;
      right: 0;
      z-index: 1000;
      background: var(--bg-card);
      border: 1px solid var(--border-color);
      border-radius: 0.5rem;
      box-shadow: var(--shadow-lg);
      max-height: 400px;
      overflow-y: auto;
      backdrop-filter: blur(10px);
    }

    :host([hidden]) {
      display: none;
    }

    .suggestions-list {
      padding: 0.5rem 0;
    }

    .suggestion-item {
      display: flex;
      align-items: center;
      padding: 0.75rem 1rem;
      cursor: pointer;
      transition: background-color 0.2s ease;
      border-bottom: 1px solid rgba(var(--border-color), 0.3);
    }

    .suggestion-item:last-child {
      border-bottom: none;
    }

    .suggestion-item:hover,
    .suggestion-item.selected {
      background: var(--bg-card-hover);
    }

    .suggestion-item.selected {
      background: rgba(var(--accent-primary), 0.1);
      border-left: 3px solid var(--accent-primary);
    }

    .suggestion-icon {
      font-size: 1rem;
      margin-right: 0.75rem;
      opacity: 0.7;
      flex-shrink: 0;
    }

    .suggestion-content {
      flex: 1;
      min-width: 0;
    }

    .suggestion-query {
      font-size: 0.9rem;
      color: var(--text-primary);
      font-weight: 500;
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }

    .suggestion-description {
      font-size: 0.75rem;
      color: var(--text-muted);
      margin-top: 0.25rem;
    }

    .suggestion-meta {
      font-size: 0.75rem;
      color: var(--text-muted);
      flex-shrink: 0;
      margin-left: 0.75rem;
    }

    .suggestion-query mark {
      background: var(--accent-warning);
      color: var(--bg-primary);
      padding: 0.1rem 0.2rem;
      border-radius: 0.2rem;
      font-weight: bold;
    }

    .no-suggestions {
      padding: 1rem;
      text-align: center;
      color: var(--text-muted);
      font-style: italic;
    }

    .section-header {
      padding: 0.5rem 1rem;
      background: var(--bg-secondary);
      color: var(--text-secondary);
      font-size: 0.75rem;
      font-weight: bold;
      text-transform: uppercase;
      letter-spacing: 0.5px;
      border-bottom: 1px solid var(--border-color);
      margin-top: 0.5rem;
    }

    .section-header:first-child {
      margin-top: 0;
    }

    .keyboard-hint {
      padding: 0.5rem 1rem;
      background: var(--bg-secondary);
      border-top: 1px solid var(--border-color);
      color: var(--text-muted);
      font-size: 0.75rem;
      text-align: center;
    }

    .keyboard-shortcut {
      background: var(--bg-primary);
      border: 1px solid var(--border-color);
      padding: 0.2rem 0.4rem;
      border-radius: 0.3rem;
      font-family: monospace;
      margin: 0 0.2rem;
    }
  `

  updated(changedProperties: Map<string, any>) {
    if (changedProperties.has('query')) {
      this._updateSuggestions()
    }
  }

  render() {
    if (!this.visible || this._suggestions.length === 0) {
      return html``
    }

    // Group suggestions by type
    const grouped = this._groupSuggestions()

    return html`
      <div class="suggestions-list">
        ${Object.entries(grouped).map(([type, suggestions]) => html`
          ${suggestions.length > 0 ? html`
            <div class="section-header">${this._getSectionTitle(type)}</div>
            ${suggestions.map((suggestion, _index) => {
              const globalIndex = this._getGlobalIndex(suggestion)
              return html`
                <div 
                  class="suggestion-item ${globalIndex === this.selectedIndex ? 'selected' : ''}"
                  @click=${() => this._selectSuggestion(suggestion)}
                  @mouseenter=${() => this._setSelectedIndex(globalIndex)}>
                  
                  <span class="suggestion-icon">
                    ${searchSuggestionsService.getSuggestionIcon(suggestion.type)}
                  </span>
                  
                  <div class="suggestion-content">
                    <div class="suggestion-query">
                      ${this._highlightQuery(suggestion.query)}
                    </div>
                    <div class="suggestion-description">
                      ${searchSuggestionsService.getSuggestionDescription(suggestion)}
                    </div>
                  </div>
                  
                  ${suggestion.count ? html`
                    <div class="suggestion-meta">${suggestion.count}</div>
                  ` : ''}
                </div>
              `
            })}
          ` : ''}
        `)}
      </div>
      
      <div class="keyboard-hint">
        <span class="keyboard-shortcut">↑↓</span> Navigate
        <span class="keyboard-shortcut">Enter</span> Select
        <span class="keyboard-shortcut">Esc</span> Close
      </div>
    `
  }

  // Update suggestions based on current query
  private _updateSuggestions() {
    this._suggestions = searchSuggestionsService.getSuggestions(this.query, 8)
    this.selectedIndex = -1
  }

  // Group suggestions by type for organized display
  private _groupSuggestions(): Record<string, SearchSuggestion[]> {
    const groups: Record<string, SearchSuggestion[]> = {
      recent: [],
      popular: [],
      tag: [],
      domain: []
    }

    this._suggestions.forEach(suggestion => {
      if (groups[suggestion.type]) {
        groups[suggestion.type].push(suggestion)
      }
    })

    return groups
  }

  // Get section title for suggestion type
  private _getSectionTitle(type: string): string {
    switch (type) {
      case 'recent': return 'Recent Searches'
      case 'popular': return 'Popular Searches'
      case 'tag': return 'Search by Tag'
      case 'domain': return 'Search by Domain'
      default: return 'Suggestions'
    }
  }

  // Get global index of suggestion in flat list
  private _getGlobalIndex(targetSuggestion: SearchSuggestion): number {
    return this._suggestions.findIndex(s => s.query === targetSuggestion.query && s.type === targetSuggestion.type)
  }

  // Highlight matching parts of the query
  private _highlightQuery(query: string): any {
    if (!this.query || !query.toLowerCase().includes(this.query.toLowerCase())) {
      return query
    }

    const regex = new RegExp(`(${this.query.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')})`, 'gi')
    const parts = query.split(regex)
    
    return parts.map((part, _index) => 
      regex.test(part) 
        ? html`<mark>${part}</mark>`
        : part
    )
  }

  // Select a suggestion
  private _selectSuggestion(suggestion: SearchSuggestion) {
    this.dispatchEvent(new CustomEvent('suggestion-selected', {
      detail: { suggestion },
      bubbles: true
    }))
  }

  // Set selected index for keyboard navigation
  private _setSelectedIndex(index: number) {
    this.selectedIndex = index
  }

  // Handle keyboard navigation
  handleKeyDown(event: KeyboardEvent) {
    if (!this.visible || this._suggestions.length === 0) {
      return false
    }

    switch (event.key) {
      case 'ArrowDown':
        event.preventDefault()
        this.selectedIndex = Math.min(this.selectedIndex + 1, this._suggestions.length - 1)
        return true

      case 'ArrowUp':
        event.preventDefault()
        this.selectedIndex = Math.max(this.selectedIndex - 1, -1)
        return true

      case 'Enter':
        event.preventDefault()
        if (this.selectedIndex >= 0 && this.selectedIndex < this._suggestions.length) {
          this._selectSuggestion(this._suggestions[this.selectedIndex])
        }
        return true

      case 'Escape':
        event.preventDefault()
        this.dispatchEvent(new CustomEvent('suggestions-close', { bubbles: true }))
        return true

      default:
        return false
    }
  }

  // Get currently selected suggestion
  getSelectedSuggestion(): SearchSuggestion | null {
    if (this.selectedIndex >= 0 && this.selectedIndex < this._suggestions.length) {
      return this._suggestions[this.selectedIndex]
    }
    return null
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'search-suggestions': SearchSuggestions
  }
}