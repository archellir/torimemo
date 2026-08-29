import { LitElement, html, css } from 'lit'
import { customElement, state } from 'lit/decorators.js'
import { searchSuggestionsService } from '../services/search-suggestions.ts'
import './search-suggestions.ts'
import './auth-dialog.ts'

// interface User { // TODO: Remove after migration
//   id: number
//   username: string
//   email: string
//   full_name?: string
//   is_admin: boolean
// }

@customElement('app-header')
export class AppHeader extends LitElement {
  @state() private _searchQuery = ''
  @state() private _showSuggestions = false
  @state() private _selectedSuggestionIndex = -1
  // @state() private _user: User | null = null // TODO: Remove after migration
  // @state() private _showUserMenu = false // TODO: Remove after migration

  static styles = css`
    :host {
      display: block;
    }

    .user-menu {
      position: absolute;
      right: 0;
      top: 100%;
      margin-top: 0.5rem;
      min-width: 200px;
    }
      transition: all 0.3s ease;
    }

    .search-input:focus {
      outline: none;
      border-color: var(--accent-primary);
      box-shadow: var(--shadow-md);
    }

    .search-input::placeholder {
      color: var(--text-muted);
    }

    .add-button {
      background: var(--bg-card);
      border: 1px solid var(--accent-primary);
      color: var(--accent-primary);
      padding: 0.75rem 1.5rem;
      border-radius: 0.5rem;
      font-family: 'Courier New', monospace;
      font-weight: bold;
      text-transform: uppercase;
      letter-spacing: 1px;
      cursor: pointer;
      transition: all 0.3s ease;
      position: relative;
      overflow: hidden;
    }

    .add-button:hover {
      background: var(--accent-primary);
      color: var(--bg-primary);
      box-shadow: var(--shadow-lg);
    }

    .add-button:before {
      content: '';
      position: absolute;
      top: 0;
      left: -100%;
      width: 100%;
      height: 100%;
      background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.2), transparent);
      transition: left 0.5s;
    }

    .add-button:hover:before {
      left: 100%;
    }

    @media (max-width: 768px) {
      .header {
        flex-direction: column;
        gap: 1rem;
        text-align: center;
      }

      .search-container {
        margin: 0;
        max-width: 100%;
      }
    }
  `

  render() {
    return html`
      <header class="header">
        <div class="logo">とりメモ</div>
        
        <div class="search-container">
          <input 
            type="text" 
            class="search-input"
            placeholder="Search bookmarks... 🔍"
            .value=${this._searchQuery}
            @input=${this._handleSearch}
            @keydown=${this._handleKeydown}
            @focus=${this._handleSearchFocus}
            @blur=${this._handleSearchBlur}
          />
          <search-suggestions
            .query=${this._searchQuery}
            .visible=${this._showSuggestions}
            .selectedIndex=${this._selectedSuggestionIndex}
            @suggestion-selected=${this._handleSuggestionSelected}
            @suggestions-close=${this._handleSuggestionsClose}
          ></search-suggestions>
        </div>
        
        <button class="add-button" @click=${this._handleAdd}>
          + Add Link
        </button>
      </header>
    `
  }

  private _handleSearch(e: Event) {
    const input = e.target as HTMLInputElement
    this._searchQuery = input.value
    this._showSuggestions = this._searchQuery.length > 0 || this._showSuggestions
    
    // Add to search history if it's a substantial query
    if (this._searchQuery.trim().length > 2) {
      searchSuggestionsService.addSearchQuery(this._searchQuery)
    }
    
    // Dispatch search event with debouncing
    this.dispatchEvent(new CustomEvent('search', {
      detail: { query: this._searchQuery }
    }))
  }

  private _handleKeydown(e: KeyboardEvent) {
    const suggestionsElement = this.shadowRoot?.querySelector('search-suggestions') as any
    
    // Let suggestions handle navigation keys first
    if (this._showSuggestions && suggestionsElement) {
      const handled = suggestionsElement.handleKeyDown(e)
      if (handled) {
        return
      }
    }
    
    if (e.key === 'Escape') {
      this._searchQuery = ''
      this._showSuggestions = false
      ;(e.target as HTMLInputElement).blur()
    } else if (e.key === 'ArrowDown' && !this._showSuggestions) {
      // Show suggestions on arrow down when not visible
      this._showSuggestions = true
    }
  }

  private _handleSearchFocus() {
    this._showSuggestions = true
  }

  private _handleSearchBlur(_e: FocusEvent) {
    // Use setTimeout to allow clicking on suggestions
    setTimeout(() => {
      this._showSuggestions = false
      this._selectedSuggestionIndex = -1
    }, 150)
  }

  private _handleSuggestionSelected(e: CustomEvent) {
    const { suggestion } = e.detail
    this._searchQuery = suggestion.query
    this._showSuggestions = false
    this._selectedSuggestionIndex = -1
    
    // Add to search history
    searchSuggestionsService.addSearchQuery(suggestion.query)
    
    // Dispatch search event
    this.dispatchEvent(new CustomEvent('search', {
      detail: { query: suggestion.query }
    }))
  }

  private _handleSuggestionsClose() {
    this._showSuggestions = false
    this._selectedSuggestionIndex = -1
  }

  private _handleAdd() {
    this.dispatchEvent(new CustomEvent('add-bookmark'))
  }
}