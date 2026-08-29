import { LitElement, html, css } from 'lit'
import { customElement, state } from 'lit/decorators.js'
// import { searchSuggestionsService } from '@services/search-suggestions.ts' // TODO: Integrate suggestions service
import '@components/search-suggestions.ts'
import '@components/auth-dialog.ts'
import type { User } from '~/types'

@customElement('app-header-new')
export class AppHeaderNew extends LitElement {
  @state() private _searchQuery = ''
  @state() private _showSuggestions = false
  @state() private _selectedSuggestionIndex = -1
  @state() private _user: User | null = null
  @state() private _showUserMenu = false

  static styles = css`
    :host {
      display: block;
    }

    .header-content {
      display: flex;
      align-items: center;
      justify-content: space-between;
      padding: 1rem;
      gap: 1rem;
    }

    .logo {
      flex-shrink: 0;
      font-size: 1.5rem;
      font-weight: bold;
      background: linear-gradient(45deg, var(--cyber-primary), var(--cyber-secondary));
      -webkit-background-clip: text;
      -webkit-text-fill-color: transparent;
      background-clip: text;
    }

    .search-container {
      flex: 1;
      max-width: 400px;
      position: relative;
      margin: 0 1rem;
    }

    .search-input {
      width: 100%;
      background: var(--bg-secondary);
      border: 1px solid var(--border-color);
      color: var(--text-primary);
      padding: 0.75rem 1rem;
      border-radius: 0.5rem;
      font-size: 0.9rem;
      transition: all 0.3s ease;
    }

    .search-input:focus {
      outline: none;
      border-color: var(--accent-primary);
      box-shadow: 0 0 0 3px rgba(var(--accent-primary), 0.1);
    }

    .actions {
      display: flex;
      align-items: center;
      gap: 0.75rem;
      flex-shrink: 0;
    }

    .add-btn {
      display: flex;
      align-items: center;
      gap: 0.5rem;
      background: transparent;
      border: 1px solid var(--accent-primary);
      color: var(--accent-primary);
      padding: 0.75rem 1rem;
      border-radius: 0.5rem;
      font-weight: 500;
      cursor: pointer;
      transition: all 0.3s ease;
      font-size: 0.9rem;
    }

    .add-btn:hover {
      background: var(--accent-primary);
      color: var(--bg-primary);
      transform: translateY(-1px);
    }

    .user-avatar {
      width: 2rem;
      height: 2rem;
      background: linear-gradient(45deg, var(--accent-primary), var(--accent-secondary));
      border-radius: 50%;
      display: flex;
      align-items: center;
      justify-content: center;
      color: var(--bg-primary);
      font-weight: bold;
      cursor: pointer;
      transition: all 0.3s ease;
    }

    .user-avatar:hover {
      transform: scale(1.1);
      box-shadow: var(--shadow-md);
    }

    .user-button {
      display: flex;
      align-items: center;
      gap: 0.5rem;
      background: transparent;
      border: none;
      color: var(--text-primary);
      cursor: pointer;
      padding: 0.5rem;
      border-radius: 0.5rem;
      transition: all 0.3s ease;
    }

    .user-button:hover {
      background: var(--bg-card-hover);
    }

    .user-menu {
      position: absolute;
      right: 0;
      top: 100%;
      margin-top: 0.5rem;
      min-width: 200px;
      z-index: 50;
      background: var(--bg-card);
      border: 1px solid var(--border-color);
      border-radius: 0.5rem;
      box-shadow: var(--shadow-lg);
      backdrop-filter: blur(10px);
    }

    .user-menu-header {
      padding: 1rem;
      border-bottom: 1px solid var(--border-color);
    }

    .user-menu-name {
      font-weight: 500;
      color: var(--text-primary);
      font-size: 0.9rem;
    }

    .user-menu-email {
      color: var(--text-secondary);
      font-size: 0.8rem;
      margin-top: 0.25rem;
    }

    .user-menu-item {
      display: flex;
      align-items: center;
      gap: 0.75rem;
      width: 100%;
      padding: 0.75rem 1rem;
      background: transparent;
      border: none;
      color: var(--text-primary);
      cursor: pointer;
      font-size: 0.9rem;
      transition: all 0.3s ease;
      text-align: left;
    }

    .user-menu-item:hover {
      background: var(--bg-card-hover);
    }

    .user-menu-item.logout {
      color: var(--accent-error);
      border-top: 1px solid var(--border-color);
    }

    .user-menu-item.logout:hover {
      background: rgba(var(--accent-error), 0.1);
    }

    .login-btn {
      display: flex;
      align-items: center;
      gap: 0.5rem;
      background: transparent;
      border: 1px solid var(--accent-secondary);
      color: var(--accent-secondary);
      padding: 0.75rem 1rem;
      border-radius: 0.5rem;
      font-weight: 500;
      cursor: pointer;
      transition: all 0.3s ease;
      font-size: 0.9rem;
    }

    .login-btn:hover {
      background: var(--accent-secondary);
      color: var(--bg-primary);
      transform: translateY(-1px);
    }

    .admin-badge {
      background: rgba(var(--accent-warning), 0.2);
      color: var(--accent-warning);
      padding: 0.125rem 0.5rem;
      border-radius: 0.25rem;
      font-size: 0.7rem;
      font-weight: 500;
      text-transform: uppercase;
    }

    .icon {
      width: 1.25rem;
      height: 1.25rem;
    }

    .icon-sm {
      width: 1rem;
      height: 1rem;
    }

    /* Mobile responsiveness */
    @media (max-width: 768px) {
      .header-content {
        padding: 0.75rem;
        gap: 0.75rem;
      }

      .logo {
        font-size: 1.25rem;
      }

      .search-container {
        margin: 0 0.5rem;
        max-width: none;
      }

      .search-input {
        font-size: 16px; /* Prevents zoom on iOS */
        padding: 0.625rem 0.75rem;
      }

      .actions {
        gap: 0.5rem;
      }

      .add-btn {
        padding: 0.625rem 0.75rem;
        font-size: 0.8rem;
      }

      .add-btn .btn-text {
        display: none; /* Hide text on mobile, keep icon */
      }

      .login-btn {
        padding: 0.625rem 0.75rem;
        font-size: 0.8rem;
      }

      .login-btn .btn-text {
        display: none; /* Hide text on mobile, keep icon */
      }

      .user-button .username {
        display: none; /* Hide username on mobile */
      }

      .user-menu {
        right: 0;
        left: auto;
        min-width: 180px;
      }
    }

    @media (max-width: 480px) {
      .header-content {
        padding: 0.5rem;
        gap: 0.5rem;
      }

      .logo {
        font-size: 1.125rem;
      }

      .search-container {
        margin: 0;
      }

      .search-input {
        padding: 0.5rem;
        font-size: 16px;
      }

      .user-avatar {
        width: 1.75rem;
        height: 1.75rem;
        font-size: 0.8rem;
      }

      .add-btn,
      .login-btn {
        padding: 0.5rem;
        min-width: 2.5rem;
        justify-content: center;
      }
    }
  `

  connectedCallback() {
    super.connectedCallback()
    this._loadUser()
    document.addEventListener('click', this._handleOutsideClick.bind(this))
  }

  disconnectedCallback() {
    super.disconnectedCallback()
    document.removeEventListener('click', this._handleOutsideClick.bind(this))
  }

  private _loadUser() {
    const token = localStorage.getItem('auth_token')
    const userStr = localStorage.getItem('user')
    
    if (token && userStr) {
      try {
        this._user = JSON.parse(userStr)
      } catch (e) {
        console.error('Failed to parse user data:', e)
        this._logout()
      }
    }
  }

  private _handleOutsideClick(e: Event) {
    const target = e.target as Element
    if (!target.closest('app-header-new')) {
      this._showUserMenu = false
    }
  }

  private _handleSearch(e: CustomEvent) {
    this._searchQuery = e.detail.query
    this._showSuggestions = false
    
    // Dispatch search event
    this.dispatchEvent(new CustomEvent('search', {
      detail: { query: this._searchQuery }
    }))
  }

  private _handleAuthSuccess(e: CustomEvent) {
    this._user = e.detail.user
    this._showUserMenu = false
  }

  private _showAuthDialog() {
    const authDialog = this.shadowRoot?.querySelector('auth-dialog')
    if (authDialog) {
      (authDialog as any).open()
    }
  }

  private async _logout() {
    try {
      const token = localStorage.getItem('auth_token')
      if (token) {
        await fetch('/api/auth/logout', {
          method: 'POST',
          headers: {
            'Authorization': `Bearer ${token}`
          }
        })
      }
    } catch (e) {
      console.error('Logout request failed:', e)
    }

    localStorage.removeItem('auth_token')
    localStorage.removeItem('user')
    this._user = null
    this._showUserMenu = false

    // Dispatch logout event
    this.dispatchEvent(new CustomEvent('auth-logout'))
  }

  private _toggleUserMenu() {
    this._showUserMenu = !this._showUserMenu
  }

  private _showAddBookmark() {
    this.dispatchEvent(new CustomEvent('add-bookmark'))
  }

  render() {
    return html`
      <div class="header-layout">
        <div class="header-content">
          <!-- Logo -->
          <div class="logo">
            📚 Torimemo
          </div>

          <!-- Search Container -->
          <div class="search-container">
            <input
              type="text"
              .value=${this._searchQuery}
              @input=${(e: Event) => {
                const target = e.target as HTMLInputElement
                this._searchQuery = target.value
                this._showSuggestions = target.value.length > 0
              }}
              @focus=${() => this._showSuggestions = this._searchQuery.length > 0}
              @blur=${() => setTimeout(() => this._showSuggestions = false, 200)}
              @keydown=${this._handleSearchKeydown}
              class="search-input"
              placeholder="Search bookmarks..."
            />
            
            ${this._showSuggestions ? html`
              <search-suggestions
                .query=${this._searchQuery}
                .selectedIndex=${this._selectedSuggestionIndex}
                @suggestion-select=${this._handleSearch}
                class="absolute w-full z-10 mt-1">
              </search-suggestions>
            ` : ''}
          </div>

          <!-- Actions -->
          <div class="actions">
            <!-- Add Bookmark Button -->
            <button @click=${this._showAddBookmark} class="add-btn">
              <svg class="icon" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4"></path>
              </svg>
              <span class="btn-text">Add</span>
            </button>

            <!-- User Menu -->
            ${this._user ? html`
              <div class="relative">
                <button @click=${this._toggleUserMenu} class="user-button">
                  <div class="user-avatar">
                    ${this._user.username.charAt(0).toUpperCase()}
                  </div>
                  <span class="username">${this._user.username}</span>
                  ${this._user.is_admin ? html`
                    <span class="admin-badge">Admin</span>
                  ` : ''}
                  <svg class="icon-sm ${this._showUserMenu ? 'rotate-180' : ''}" 
                       fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"></path>
                  </svg>
                </button>

                ${this._showUserMenu ? html`
                  <div class="user-menu">
                    <div class="user-menu-header">
                      <div class="user-menu-name">${this._user.username}</div>
                      <div class="user-menu-email">${this._user.email}</div>
                    </div>
                    
                    <button class="user-menu-item">
                      <svg class="icon-sm" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" 
                              d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z"></path>
                      </svg>
                      Profile
                    </button>
                    
                    <button class="user-menu-item">
                      <svg class="icon-sm" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" 
                              d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z"></path>
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"></path>
                      </svg>
                      Settings
                    </button>
                    
                    <button @click=${this._logout} class="user-menu-item logout">
                      <svg class="icon-sm" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" 
                              d="M17 16l4-4m0 0l-4-4m4 4H7m6 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h4a3 3 0 013 3v1"></path>
                      </svg>
                      Logout
                    </button>
                  </div>
                ` : ''}
              </div>
            ` : html`
              <button @click=${this._showAuthDialog} class="login-btn">
                <svg class="icon" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" 
                        d="M11 16l-4-4m0 0l4-4m-4 4h14m-5 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h4a3 3 0 013 3v1"></path>
                </svg>
                <span class="btn-text">Login</span>
              </button>
            `}
          </div>
        </div>
      </div>

      <auth-dialog @auth-success=${this._handleAuthSuccess}></auth-dialog>
    `
  }

  private _handleSearchKeydown = (e: KeyboardEvent) => {
    if (e.key === 'Escape') {
      this._showSuggestions = false
    } else if (e.key === 'Enter') {
      this._showSuggestions = false
      this.dispatchEvent(new CustomEvent('search', {
        detail: { query: this._searchQuery }
      }))
    }
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'app-header-new': AppHeaderNew
  }
}