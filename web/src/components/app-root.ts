import { LitElement, html, css } from 'lit'
import { customElement, state } from 'lit/decorators.js'
import type { Bookmark } from '../services/api.ts'
import './app-header-new.ts'
import './bookmark-list.ts'
import './bookmark-dialog.ts'
import './tag-cloud.ts'
import './theme-toggle.ts'
import './import-dialog.ts'
import './export-dialog.ts'
import './advanced-search.ts'
import './folder-tree.ts'
import './folder-dialog.ts'
import { searchSuggestionsService } from '../services/search-suggestions.ts'
import { archiveService } from '../services/archive.ts'

@customElement('app-root')
export class AppRoot extends LitElement {
  @state() private _showDialog = false
  @state() private _editBookmark: Bookmark | null = null
  @state() private _searchQuery = ''
  @state() private _tagFilter = ''
  @state() private _favoritesOnly = false
  @state() private _showHelp = false
  @state() private _showImportDialog = false
  @state() private _showExportDialog = false
  @state() private _availableTags: string[] = []
  @state() private _availableDomains: string[] = []
  @state() private _advancedFilters: any = null
  @state() private _showFolderDialog = false
  @state() private _editingFolder: any = null
  @state() private _parentFolder: any = null
  @state() private _selectedFolderId = -1

  connectedCallback() {
    super.connectedCallback();
    // Initialize theme manager to apply theme classes
    import('../services/theme.js').then(({ themeManager }) => {
      const updateTheme = () => {
        const theme = themeManager.getCurrentTheme();
        const actualTheme = theme === 'auto' 
          ? (window.matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark')
          : theme;
        this.className = actualTheme;
      };
      
      updateTheme();
      themeManager.subscribe(updateTheme);
    });
    
    // Setup keyboard navigation
    this._setupKeyboardNavigation();
    
    // Load data for advanced search
    this._loadAvailableTagsAndDomains();
  }

  disconnectedCallback() {
    super.disconnectedCallback();
    document.removeEventListener('keydown', this._handleKeydown);
  }

  private _setupKeyboardNavigation() {
    document.addEventListener('keydown', this._handleKeydown);
  }

  private _handleKeydown = (e: KeyboardEvent) => {
    // Ignore if typing in input fields
    if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) {
      return;
    }

    // Global keyboard shortcuts
    if (e.key === '/' || (e.key === 'k' && (e.metaKey || e.ctrlKey))) {
      // Focus search (/ or Cmd/Ctrl+K)
      e.preventDefault();
      this._focusSearch();
    } else if (e.key === 'n' && (e.metaKey || e.ctrlKey)) {
      // New bookmark (Cmd/Ctrl+N)
      e.preventDefault();
      this._handleAddBookmark();
    } else if (e.key === 'Escape') {
      // Close dialog, help panel, or clear search
      e.preventDefault();
      if (this._showDialog) {
        this._handleCloseDialog();
      } else if (this._showHelp) {
        this._showHelp = false;
      } else if (this._showImportDialog) {
        this._showImportDialog = false;
      } else {
        this._clearSearch();
      }
    } else if (e.key === 't' && (e.metaKey || e.ctrlKey)) {
      // Toggle theme (Cmd/Ctrl+T)
      e.preventDefault();
      this._toggleTheme();
    } else if (e.key === 'f' && (e.metaKey || e.ctrlKey)) {
      // Toggle favorites filter (Cmd/Ctrl+F)
      e.preventDefault();
      this._favoritesOnly = !this._favoritesOnly;
    } else if (e.key === '?' || (e.key === 'h' && (e.metaKey || e.ctrlKey))) {
      // Show help (? or Cmd/Ctrl+H)
      e.preventDefault();
      this._showHelp = !this._showHelp;
    } else if (e.key === 'i' && (e.metaKey || e.ctrlKey)) {
      // Show import dialog (Cmd/Ctrl+I)
      e.preventDefault();
      this._showImportDialog = true;
    }
  }

  private _focusSearch() {
    const searchInput = this.shadowRoot?.querySelector('app-header-new')?.shadowRoot?.querySelector('.search-input') as HTMLInputElement;
    if (searchInput) {
      searchInput.focus();
      searchInput.select();
    }
  }

  private _clearSearch() {
    const searchInput = this.shadowRoot?.querySelector('app-header-new')?.shadowRoot?.querySelector('.search-input') as HTMLInputElement;
    if (searchInput) {
      searchInput.value = '';
      searchInput.dispatchEvent(new Event('input', { bubbles: true }));
      searchInput.blur();
    }
  }

  private _toggleTheme() {
    const themeToggle = this.shadowRoot?.querySelector('theme-toggle') as any;
    if (themeToggle?.toggleThemeViaKeyboard) {
      themeToggle.toggleThemeViaKeyboard();
    }
  }

  static styles = css`
    :host {
      display: block;
      min-height: 100vh;
      background: var(--bg-primary);
      color: var(--text-primary);
      transition: background-color 0.3s ease, color 0.3s ease;
    }

    /* Dark theme backgrounds (cyberpunk) */
    :host(.dark) {
      background: 
        radial-gradient(circle at 20% 50%, rgba(0, 255, 255, 0.1) 0%, transparent 50%),
        radial-gradient(circle at 80% 20%, rgba(255, 0, 128, 0.1) 0%, transparent 50%),
        linear-gradient(135deg, #0a0a0a 0%, #1a1a1a 100%);
    }

    /* Light theme backgrounds */
    :host(.light) {
      background: 
        radial-gradient(circle at 20% 50%, rgba(13, 110, 253, 0.05) 0%, transparent 50%),
        radial-gradient(circle at 80% 20%, rgba(111, 66, 193, 0.05) 0%, transparent 50%),
        linear-gradient(135deg, #f8f9fa 0%, #ffffff 100%);
    }

    .container {
      max-width: 1200px;
      margin: 0 auto;
      padding: 2rem;
    }

    .main-content {
      display: grid;
      grid-template-columns: 1fr;
      gap: 2rem;
      margin-top: 2rem;
    }

    @media (min-width: 768px) {
      .main-content {
        grid-template-columns: 300px 1fr;
      }
    }

    .sidebar {
      background: var(--bg-card);
      border: 1px solid var(--border-color);
      border-radius: 0.5rem;
      padding: 1.5rem;
      backdrop-filter: blur(10px);
      height: fit-content;
      box-shadow: var(--shadow-md);
    }

    .sidebar-title {
      color: var(--accent-primary);
      font-size: 1.25rem;
      font-weight: bold;
      margin-bottom: 1rem;
      text-shadow: var(--shadow-sm);
    }

    .theme-container {
      margin-bottom: 1.5rem;
      padding-bottom: 1rem;
      border-bottom: 1px solid var(--border-color);
    }

    .filter-group {
      margin-bottom: 1.5rem;
    }

    .filter-label {
      display: block;
      color: var(--text-secondary);
      font-size: 0.875rem;
      font-weight: 500;
      text-transform: uppercase;
      letter-spacing: 0.5px;
      margin-bottom: 0.5rem;
    }

    .filter-toggle {
      display: flex;
      align-items: center;
      gap: 0.5rem;
      cursor: pointer;
      padding: 0.5rem;
      border-radius: 0.25rem;
      transition: background-color 0.3s ease;
      color: var(--text-primary);
    }

    .filter-toggle:hover {
      background: var(--bg-card-hover);
    }

    .filter-toggle input[type="checkbox"] {
      width: 16px;
      height: 16px;
      accent-color: var(--accent-primary);
    }

    .import-button,
    .export-button {
      width: 100%;
      background: var(--bg-card);
      border: 1px solid var(--accent-secondary);
      color: var(--accent-secondary);
      padding: 0.75rem 1rem;
      border-radius: 0.5rem;
      font-family: 'Courier New', monospace;
      font-weight: bold;
      cursor: pointer;
      transition: all 0.3s ease;
      text-transform: uppercase;
      letter-spacing: 0.5px;
    }

    .import-button:hover,
    .export-button:hover {
      background: var(--accent-secondary);
      color: var(--bg-primary);
      box-shadow: var(--shadow-md);
    }

    .content {
      background: var(--bg-card);
      border: 1px solid var(--border-color);
      border-radius: 0.5rem;
      padding: 1.5rem;
      backdrop-filter: blur(10px);
      box-shadow: var(--shadow-md);
    }

    .welcome-message {
      text-align: center;
      padding: 3rem 1rem;
      color: var(--text-muted);
    }

    .welcome-title {
      font-size: 2rem;
      font-weight: bold;
      margin-bottom: 1rem;
      background: linear-gradient(45deg, var(--accent-primary), var(--accent-secondary));
      -webkit-background-clip: text;
      -webkit-text-fill-color: transparent;
      background-clip: text;
    }

    .quick-actions {
      background: rgba(var(--accent-warning), 0.05);
      border: 1px solid rgba(var(--accent-warning), 0.2);
      border-radius: 0.5rem;
      padding: 1rem;
      margin-bottom: 1.5rem;
    }

    .quick-actions-title {
      color: var(--accent-warning);
      font-size: 1rem;
      font-weight: bold;
      margin-bottom: 0.5rem;
    }

    .quick-actions-text {
      color: var(--text-muted);
      font-size: 0.875rem;
      line-height: 1.4;
    }

    .help-overlay {
      position: fixed;
      top: 0;
      left: 0;
      right: 0;
      bottom: 0;
      background: rgba(0, 0, 0, 0.8);
      backdrop-filter: blur(10px);
      display: flex;
      align-items: center;
      justify-content: center;
      z-index: 1001;
    }

    .help-panel {
      background: var(--bg-card);
      border: 1px solid var(--border-color);
      border-radius: 1rem;
      padding: 2rem;
      width: 90%;
      max-width: 600px;
      max-height: 80vh;
      overflow-y: auto;
      box-shadow: var(--shadow-lg);
    }

    .help-title {
      font-size: 1.5rem;
      font-weight: bold;
      color: var(--accent-primary);
      margin-bottom: 1.5rem;
      text-align: center;
    }

    .help-section {
      margin-bottom: 2rem;
    }

    .help-section-title {
      font-size: 1.125rem;
      font-weight: bold;
      color: var(--text-primary);
      margin-bottom: 1rem;
      border-bottom: 1px solid var(--border-color);
      padding-bottom: 0.5rem;
    }

    .help-shortcuts {
      display: grid;
      gap: 0.75rem;
    }

    .help-shortcut {
      display: flex;
      justify-content: space-between;
      align-items: center;
      padding: 0.5rem;
      background: var(--bg-secondary);
      border-radius: 0.5rem;
    }

    .help-keys {
      display: flex;
      gap: 0.25rem;
    }

    .help-key {
      background: var(--bg-tertiary);
      border: 1px solid var(--border-color);
      border-radius: 0.25rem;
      padding: 0.25rem 0.5rem;
      font-family: 'Courier New', monospace;
      font-size: 0.75rem;
      color: var(--text-secondary);
    }

    .help-description {
      color: var(--text-primary);
      font-size: 0.875rem;
    }

    .help-close {
      background: var(--accent-primary);
      color: var(--bg-primary);
      border: none;
      padding: 0.75rem 1.5rem;
      border-radius: 0.5rem;
      font-weight: bold;
      cursor: pointer;
      margin-top: 1.5rem;
      width: 100%;
    }

    /* Mobile responsiveness */
    @media (max-width: 1024px) {
      .container {
        padding: 1.5rem;
      }
      
      .main-content {
        gap: 1.5rem;
      }
    }

    @media (max-width: 768px) {
      .container {
        padding: 1rem;
      }
      
      .main-content {
        grid-template-columns: 1fr;
        gap: 1rem;
      }
      
      .sidebar {
        order: 2;
        padding: 1rem;
        margin-top: 0;
      }
      
      .content {
        order: 1;
        padding: 1rem;
      }

      .help-shortcut {
        flex-direction: column;
        align-items: flex-start;
        gap: 0.5rem;
      }
      
      .help-panel {
        padding: 1.5rem;
        width: 95%;
        max-height: 85vh;
      }
      
      .help-title {
        font-size: 1.25rem;
      }
      
      .quick-actions {
        padding: 0.75rem;
        margin-bottom: 1rem;
      }
      
      .filter-group {
        margin-bottom: 1rem;
      }
    }

    @media (max-width: 480px) {
      .container {
        padding: 0.75rem;
      }
      
      .main-content {
        gap: 0.75rem;
        margin-top: 1rem;
      }
      
      .sidebar {
        padding: 0.75rem;
        border-radius: 0.375rem;
      }
      
      .content {
        padding: 0.75rem;
        border-radius: 0.375rem;
      }
      
      .sidebar-title {
        font-size: 1.125rem;
        margin-bottom: 0.75rem;
      }
      
      .filter-toggle {
        padding: 0.375rem;
        font-size: 0.9rem;
      }
      
      .import-button,
      .export-button {
        padding: 0.625rem 0.75rem;
        font-size: 0.8rem;
      }
      
      .welcome-title {
        font-size: 1.5rem;
      }
      
      .welcome-message {
        padding: 2rem 0.75rem;
      }
      
      .help-panel {
        padding: 1rem;
        border-radius: 0.75rem;
      }
      
      .help-title {
        font-size: 1.125rem;
        margin-bottom: 1rem;
      }
      
      .help-section {
        margin-bottom: 1.5rem;
      }
      
      .help-section-title {
        font-size: 1rem;
        margin-bottom: 0.75rem;
      }
      
      .help-close {
        padding: 0.625rem 1.25rem;
        font-size: 0.9rem;
        margin-top: 1rem;
      }
    }
  `

  render() {
    return html`
      <div class="container">
        <app-header-new 
          @search=${this._handleSearch}
          @add-bookmark=${this._handleAddBookmark}
          @auth-success=${this._handleAuthSuccess}
          @auth-logout=${this._handleAuthLogout}>
        </app-header-new>
        
        <div class="main-content">
          <aside class="sidebar">
            <div class="theme-container">
              <label class="filter-label">Theme</label>
              <theme-toggle></theme-toggle>
            </div>
            
            <h3 class="sidebar-title">Filters</h3>
            
            <div class="filter-group">
              <label class="filter-label">Quick Filters</label>
              <label class="filter-toggle">
                <input 
                  type="checkbox" 
                  .checked=${this._favoritesOnly}
                  @change=${this._handleFavoritesToggle}
                />
                <span>⭐ Favorites Only</span>
              </label>
            </div>

            <div class="filter-group">
              <label class="filter-label">Tags</label>
              <tag-cloud 
                .selectedTag=${this._tagFilter}
                @tag-selected=${this._handleTagSelected}
                @tag-cleared=${this._handleTagCleared}>
              </tag-cloud>
            </div>

            <div class="filter-group">
              <label class="filter-label">Actions</label>
              <button class="import-button" @click=${this._handleShowImport}>
                📥 Import Bookmarks
              </button>
              <button class="export-button" @click=${this._handleShowExport}>
                📤 Export Bookmarks
              </button>
            </div>

            <div class="filter-group">
              <label class="filter-label">Folders</label>
              <folder-tree 
                .allowSelection=${true}
                .selectedFolderId=${this._selectedFolderId}
                @folder-selected=${this._handleFolderSelected}
                @folder-create=${this._handleFolderCreate}
                @folder-edit=${this._handleFolderEdit}
                @folder-delete=${this._handleFolderDelete}>
              </folder-tree>
            </div>

            <div class="quick-actions">
              <div class="quick-actions-title">💡 Pro Tip</div>
              <div class="quick-actions-text">
                Use the search box to find bookmarks instantly, or click the "+" button to add new ones with AI-powered tagging!
              </div>
            </div>
          </aside>
          
          <main class="content">
            <advanced-search
              .availableTags=${this._availableTags}
              .availableDomains=${this._availableDomains}
              @filters-changed=${this._handleAdvancedFiltersChanged}>
            </advanced-search>
            
            <bookmark-list 
              .searchQuery=${this._advancedFilters ? '' : this._searchQuery}
              .tagFilter=${this._advancedFilters ? '' : this._tagFilter}
              .favoritesOnly=${this._advancedFilters ? false : this._favoritesOnly}
              .advancedFilters=${this._advancedFilters}
              @edit-bookmark=${this._handleEditBookmark}
              @bookmark-deleted=${this._handleBookmarkDeleted}>
            </bookmark-list>
          </main>
        </div>
        
        ${this._showDialog ? html`
          <bookmark-dialog 
            .editBookmark=${this._editBookmark}
            @close=${this._handleCloseDialog}
            @save=${this._handleSaveBookmark}>
          </bookmark-dialog>
        ` : ''}

        ${this._showHelp ? html`
          <div class="help-overlay" @click=${this._handleHelpOverlayClick}>
            <div class="help-panel" @click=${(e: Event) => e.stopPropagation()}>
              <h2 class="help-title">⌨️ Keyboard Shortcuts</h2>
              
              <div class="help-section">
                <h3 class="help-section-title">Global</h3>
                <div class="help-shortcuts">
                  <div class="help-shortcut">
                    <div class="help-keys">
                      <span class="help-key">/</span>
                      <span>or</span>
                      <span class="help-key">Cmd</span>
                      <span class="help-key">K</span>
                    </div>
                    <span class="help-description">Focus search</span>
                  </div>
                  <div class="help-shortcut">
                    <div class="help-keys">
                      <span class="help-key">Cmd</span>
                      <span class="help-key">N</span>
                    </div>
                    <span class="help-description">Add new bookmark</span>
                  </div>
                  <div class="help-shortcut">
                    <div class="help-keys">
                      <span class="help-key">Cmd</span>
                      <span class="help-key">I</span>
                    </div>
                    <span class="help-description">Import bookmarks</span>
                  </div>
                  <div class="help-shortcut">
                    <div class="help-keys">
                      <span class="help-key">Cmd</span>
                      <span class="help-key">T</span>
                    </div>
                    <span class="help-description">Toggle theme</span>
                  </div>
                  <div class="help-shortcut">
                    <div class="help-keys">
                      <span class="help-key">Cmd</span>
                      <span class="help-key">F</span>
                    </div>
                    <span class="help-description">Toggle favorites filter</span>
                  </div>
                  <div class="help-shortcut">
                    <div class="help-keys">
                      <span class="help-key">Escape</span>
                    </div>
                    <span class="help-description">Close dialog / Clear search</span>
                  </div>
                  <div class="help-shortcut">
                    <div class="help-keys">
                      <span class="help-key">?</span>
                    </div>
                    <span class="help-description">Show this help</span>
                  </div>
                </div>
              </div>

              <div class="help-section">
                <h3 class="help-section-title">Bookmark List</h3>
                <div class="help-shortcuts">
                  <div class="help-shortcut">
                    <div class="help-keys">
                      <span class="help-key">↑</span>
                      <span class="help-key">↓</span>
                      <span>or</span>
                      <span class="help-key">J</span>
                      <span class="help-key">K</span>
                    </div>
                    <span class="help-description">Navigate bookmarks</span>
                  </div>
                  <div class="help-shortcut">
                    <div class="help-keys">
                      <span class="help-key">Enter</span>
                    </div>
                    <span class="help-description">Open bookmark</span>
                  </div>
                  <div class="help-shortcut">
                    <div class="help-keys">
                      <span class="help-key">F</span>
                    </div>
                    <span class="help-description">Toggle favorite</span>
                  </div>
                  <div class="help-shortcut">
                    <div class="help-keys">
                      <span class="help-key">E</span>
                    </div>
                    <span class="help-description">Edit bookmark</span>
                  </div>
                  <div class="help-shortcut">
                    <div class="help-keys">
                      <span class="help-key">D</span>
                    </div>
                    <span class="help-description">Delete bookmark</span>
                  </div>
                </div>
              </div>

              <div class="help-section">
                <h3 class="help-section-title">Tag Navigation</h3>
                <div class="help-shortcuts">
                  <div class="help-shortcut">
                    <div class="help-keys">
                      <span class="help-key">←</span>
                      <span class="help-key">→</span>
                      <span>or</span>
                      <span class="help-key">H</span>
                      <span class="help-key">L</span>
                    </div>
                    <span class="help-description">Navigate tags</span>
                  </div>
                  <div class="help-shortcut">
                    <div class="help-keys">
                      <span class="help-key">Enter</span>
                    </div>
                    <span class="help-description">Select tag filter</span>
                  </div>
                  <div class="help-shortcut">
                    <div class="help-keys">
                      <span class="help-key">Escape</span>
                    </div>
                    <span class="help-description">Clear tag filter</span>
                  </div>
                </div>
              </div>

              <button class="help-close" @click=${this._closeHelp}>
                Got it!
              </button>
            </div>
          </div>
        ` : ''}

        ${this._showImportDialog ? html`
          <import-dialog 
            @close=${this._handleCloseImport}
            @import-success=${this._handleImportSuccess}>
          </import-dialog>
        ` : ''}

        ${this._showExportDialog ? html`
          <export-dialog 
            @close=${this._handleCloseExport}>
          </export-dialog>
        ` : ''}

        ${this._showFolderDialog ? html`
          <folder-dialog
            .open=${this._showFolderDialog}
            .folder=${this._editingFolder}
            .parentFolder=${this._parentFolder}
            @dialog-close=${this._handleCloseFolderDialog}
            @folder-saved=${this._handleFolderSaved}>
          </folder-dialog>
        ` : ''}
      </div>
    `
  }

  private _handleSearch(e: CustomEvent) {
    this._searchQuery = e.detail.query || ''
    
    // If offline and there's a search query, also search cached content
    if (archiveService.isOffline() && this._searchQuery.trim()) {
      this._searchOfflineContent(this._searchQuery)
    }
  }

  private async _searchOfflineContent(query: string) {
    try {
      const results = await archiveService.searchCached(query)
      if (results.length > 0) {
        // Could show these in a separate offline results section
        console.log('Found', results.length, 'cached results for offline search')
      }
    } catch (error) {
      console.error('Offline search failed:', error)
    }
  }

  private _handleFavoritesToggle(e: Event) {
    const checkbox = e.target as HTMLInputElement
    this._favoritesOnly = checkbox.checked
  }

  private _handleAddBookmark() {
    this._editBookmark = null
    this._showDialog = true
  }

  private _handleEditBookmark(e: CustomEvent) {
    this._editBookmark = e.detail.bookmark
    this._showDialog = true
  }

  private _handleCloseDialog() {
    this._showDialog = false
    this._editBookmark = null
  }

  private _handleSaveBookmark(e: CustomEvent) {
    const { bookmark, isEdit } = e.detail
    
    // Close dialog
    this._showDialog = false
    this._editBookmark = null
    
    // Force bookmark list to refresh by dispatching an event
    this.shadowRoot?.querySelector('bookmark-list')?.dispatchEvent(
      new CustomEvent('refresh-needed')
    )
    
    // Refresh tag cloud to update counts
    const tagCloud = this.shadowRoot?.querySelector('tag-cloud') as any
    tagCloud?.loadTags()
    
    // Could show a success toast here
    console.log(isEdit ? 'Bookmark updated:' : 'Bookmark created:', bookmark)
  }

  private _handleTagSelected(e: CustomEvent) {
    this._tagFilter = e.detail.tag
  }

  private _handleTagCleared() {
    this._tagFilter = ''
  }

  private _handleBookmarkDeleted() {
    // Refresh tag cloud when bookmark is deleted
    const tagCloud = this.shadowRoot?.querySelector('tag-cloud') as any
    tagCloud?.loadTags()
  }

  private _handleHelpOverlayClick() {
    this._showHelp = false
  }

  private _closeHelp() {
    this._showHelp = false
  }

  private _handleShowImport() {
    this._showImportDialog = true
  }

  private _handleCloseImport() {
    this._showImportDialog = false
  }

  private _handleShowExport() {
    this._showExportDialog = true
  }

  private _handleCloseExport() {
    this._showExportDialog = false
  }

  // Folder handlers
  private _handleFolderSelected(e: CustomEvent) {
    this._selectedFolderId = e.detail.folder.id
    // TODO: Filter bookmarks by folder
  }

  private _handleFolderCreate(e: CustomEvent) {
    this._editingFolder = null
    this._parentFolder = e.detail?.parentFolder || null
    this._showFolderDialog = true
  }

  private _handleFolderEdit(e: CustomEvent) {
    this._editingFolder = e.detail.folder
    this._parentFolder = null
    this._showFolderDialog = true
  }

  private async _handleFolderDelete(e: CustomEvent) {
    const folder = e.detail.folder
    try {
      // Import API service at the top if not already imported
      const { apiService } = await import('../services/api.ts')
      await apiService.deleteFolder(folder.id)
      
      // Refresh folder tree
      const folderTree = this.shadowRoot?.querySelector('folder-tree') as any
      if (folderTree) {
        await folderTree.refresh()
      }
    } catch (error) {
      console.error('Failed to delete folder:', error)
    }
  }

  private _handleCloseFolderDialog() {
    this._showFolderDialog = false
    this._editingFolder = null
    this._parentFolder = null
  }

  private async _handleFolderSaved(e: CustomEvent) {
    console.log('Folder saved:', e.detail.folder)
    
    // Refresh folder tree
    const folderTree = this.shadowRoot?.querySelector('folder-tree') as any
    if (folderTree) {
      await folderTree.refresh()
    }
  }

  private _handleAdvancedFiltersChanged(e: CustomEvent) {
    this._advancedFilters = e.detail.filters
    // Clear basic search when using advanced search
    this._searchQuery = ''
    this._tagFilter = ''
    this._favoritesOnly = false
  }

  private _handleImportSuccess(e: CustomEvent) {
    // Close import dialog
    this._showImportDialog = false
    
    // Refresh bookmark list and tag cloud
    const bookmarkList = this.shadowRoot?.querySelector('bookmark-list') as any
    bookmarkList?.loadBookmarks(true)
    
    const tagCloud = this.shadowRoot?.querySelector('tag-cloud') as any
    tagCloud?.loadTags()
    
    // Refresh available tags and domains for advanced search
    this._loadAvailableTagsAndDomains()
    
    console.log('Import completed:', e.detail)
  }

  private async _loadAvailableTagsAndDomains() {
    try {
      // Load available tags
      const tagsResponse = await fetch('/api/tags')
      if (tagsResponse.ok) {
        const tags = await tagsResponse.json()
        this._availableTags = tags.map((tag: any) => tag.name)
      }

      // Load available domains from bookmarks
      const bookmarksResponse = await fetch('/api/bookmarks?limit=1000')
      if (bookmarksResponse.ok) {
        const data = await bookmarksResponse.json()
        const domains = new Set<string>()
        data.bookmarks.forEach((bookmark: any) => {
          try {
            const url = new URL(bookmark.url)
            domains.add(url.hostname)
          } catch {
            // Skip invalid URLs
          }
        })
        this._availableDomains = Array.from(domains).sort()
      }

      // Update search suggestions service with metadata
      searchSuggestionsService.updateMetadata(this._availableTags, this._availableDomains)
    } catch (error) {
      console.error('Failed to load tags and domains:', error)
    }
  }

  private async _handleAuthSuccess(e: CustomEvent) {
    const { user, token } = e.detail
    
    // Store authentication data
    localStorage.setItem('auth_token', token)
    localStorage.setItem('user', JSON.stringify(user))
    
    // Refresh bookmark list to show user's bookmarks
    const bookmarkList = this.shadowRoot?.querySelector('bookmark-list') as any
    if (bookmarkList) {
      await bookmarkList.loadBookmarks(true)
    }
    
    // Refresh tag cloud and folder tree
    const tagCloud = this.shadowRoot?.querySelector('tag-cloud') as any
    if (tagCloud) {
      await tagCloud.loadTags()
    }
    
    const folderTree = this.shadowRoot?.querySelector('folder-tree') as any
    if (folderTree) {
      await folderTree.refresh()
    }
    
    // Refresh available data for advanced search
    await this._loadAvailableTagsAndDomains()
    
    console.log('Authentication successful for user:', user.username)
  }

  private async _handleAuthLogout() {
    // Clear authentication data
    localStorage.removeItem('auth_token')
    localStorage.removeItem('user')
    
    // Reset all filters and search
    this._searchQuery = ''
    this._tagFilter = ''
    this._favoritesOnly = false
    this._advancedFilters = null
    this._selectedFolderId = -1
    
    // Refresh all data-dependent components to show public/empty state
    const bookmarkList = this.shadowRoot?.querySelector('bookmark-list') as any
    if (bookmarkList) {
      await bookmarkList.loadBookmarks(true)
    }
    
    const tagCloud = this.shadowRoot?.querySelector('tag-cloud') as any
    if (tagCloud) {
      await tagCloud.loadTags()
    }
    
    const folderTree = this.shadowRoot?.querySelector('folder-tree') as any
    if (folderTree) {
      await folderTree.refresh()
    }
    
    // Clear available data arrays
    this._availableTags = []
    this._availableDomains = []
    
    console.log('User logged out successfully')
  }
}