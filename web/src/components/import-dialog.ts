import { LitElement, html, css } from 'lit'
import { customElement, state } from 'lit/decorators.js'

export interface ImportResult {
  success: boolean
  imported: number
  skipped: number
  errors: number
  message: string
}

@customElement('import-dialog')
export class ImportDialog extends LitElement {
  @state() private _selectedFormat = 'chrome'
  @state() private _fileContent = ''
  @state() private _fileName = ''
  @state() private _isImporting = false
  @state() private _error: string | null = null
  @state() private _result: ImportResult | null = null

  static styles = css`
    :host {
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
      z-index: 1000;
    }

    .dialog {
      background: var(--bg-card);
      border: 1px solid var(--border-color);
      border-radius: 1rem;
      padding: 2rem;
      width: 90%;
      max-width: 600px;
      max-height: 80vh;
      overflow-y: auto;
      box-shadow: var(--shadow-lg);
      position: relative;
    }

    .dialog:before {
      content: '';
      position: absolute;
      top: 0;
      left: 0;
      right: 0;
      height: 2px;
      background: linear-gradient(90deg, var(--accent-primary), var(--accent-secondary), var(--accent-warning));
    }

    .dialog-header {
      display: flex;
      align-items: center;
      justify-content: space-between;
      margin-bottom: 1.5rem;
    }

    .dialog-title {
      font-size: 1.25rem;
      font-weight: bold;
      color: var(--accent-primary);
      text-shadow: var(--shadow-sm);
    }

    .close-button {
      background: none;
      border: none;
      color: var(--text-muted);
      font-size: 1.5rem;
      cursor: pointer;
      padding: 0.25rem;
      border-radius: 0.25rem;
      transition: all 0.3s ease;
    }

    .close-button:hover {
      color: var(--accent-danger);
      background: rgba(var(--accent-danger), 0.1);
    }

    .form-section {
      margin-bottom: 2rem;
    }

    .section-title {
      font-size: 1rem;
      font-weight: bold;
      color: var(--text-primary);
      margin-bottom: 1rem;
      border-bottom: 1px solid var(--border-color);
      padding-bottom: 0.5rem;
    }

    .format-options {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
      gap: 1rem;
      margin-bottom: 1.5rem;
    }

    .format-option {
      display: flex;
      align-items: center;
      padding: 1rem;
      background: var(--bg-secondary);
      border: 1px solid var(--border-color);
      border-radius: 0.5rem;
      cursor: pointer;
      transition: all 0.3s ease;
    }

    .format-option:hover {
      background: var(--bg-card-hover);
      border-color: var(--accent-primary);
    }

    .format-option.selected {
      background: rgba(var(--accent-primary), 0.1);
      border-color: var(--accent-primary);
      box-shadow: var(--shadow-md);
    }

    .format-radio {
      margin-right: 0.75rem;
      accent-color: var(--accent-primary);
    }

    .format-info {
      flex: 1;
    }

    .format-name {
      font-weight: bold;
      color: var(--text-primary);
      margin-bottom: 0.25rem;
    }

    .format-description {
      font-size: 0.875rem;
      color: var(--text-secondary);
    }

    .file-upload {
      position: relative;
      margin-bottom: 1rem;
    }

    .file-input {
      position: absolute;
      opacity: 0;
      width: 100%;
      height: 100%;
      cursor: pointer;
    }

    .file-button {
      display: flex;
      align-items: center;
      justify-content: center;
      gap: 0.5rem;
      width: 100%;
      padding: 1rem;
      background: var(--bg-secondary);
      border: 2px dashed var(--border-color);
      border-radius: 0.5rem;
      color: var(--text-secondary);
      transition: all 0.3s ease;
      cursor: pointer;
    }

    .file-button:hover {
      background: var(--bg-card-hover);
      border-color: var(--accent-primary);
      color: var(--text-primary);
    }

    .file-button.has-file {
      background: rgba(var(--accent-success), 0.1);
      border-color: var(--accent-success);
      color: var(--accent-success);
    }

    .file-name {
      font-weight: bold;
      margin-top: 0.5rem;
      color: var(--text-primary);
      text-align: center;
    }

    .instructions {
      background: rgba(var(--accent-warning), 0.05);
      border: 1px solid rgba(var(--accent-warning), 0.2);
      border-radius: 0.5rem;
      padding: 1rem;
      margin-bottom: 1.5rem;
    }

    .instructions-title {
      color: var(--accent-warning);
      font-weight: bold;
      margin-bottom: 0.5rem;
    }

    .instructions-text {
      color: var(--text-secondary);
      font-size: 0.875rem;
      line-height: 1.4;
    }

    .instructions ul {
      margin: 0.5rem 0;
      padding-left: 1.5rem;
    }

    .error-message {
      background: rgba(var(--accent-danger), 0.1);
      border: 1px solid rgba(var(--accent-danger), 0.3);
      color: var(--accent-danger);
      padding: 1rem;
      border-radius: 0.5rem;
      margin-bottom: 1rem;
      text-align: center;
    }

    .result-message {
      background: rgba(var(--accent-success), 0.1);
      border: 1px solid rgba(var(--accent-success), 0.3);
      color: var(--accent-success);
      padding: 1rem;
      border-radius: 0.5rem;
      margin-bottom: 1rem;
    }

    .result-stats {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(100px, 1fr));
      gap: 1rem;
      margin-top: 1rem;
    }

    .result-stat {
      text-align: center;
      padding: 0.5rem;
      background: var(--bg-secondary);
      border-radius: 0.5rem;
    }

    .result-stat-number {
      font-size: 1.5rem;
      font-weight: bold;
      color: var(--accent-primary);
    }

    .result-stat-label {
      font-size: 0.75rem;
      color: var(--text-secondary);
      text-transform: uppercase;
      letter-spacing: 0.5px;
    }

    .dialog-actions {
      display: flex;
      gap: 1rem;
      margin-top: 2rem;
    }

    .button {
      flex: 1;
      background: transparent;
      border: 1px solid;
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

    .button:disabled {
      opacity: 0.5;
      cursor: not-allowed;
    }

    .button-primary {
      border-color: var(--accent-primary);
      color: var(--accent-primary);
      background: var(--bg-card);
    }

    .button-primary:hover:not(:disabled) {
      background: var(--accent-primary);
      color: var(--bg-primary);
      box-shadow: var(--shadow-lg);
    }

    .button-secondary {
      border-color: var(--text-muted);
      color: var(--text-muted);
    }

    .button-secondary:hover:not(:disabled) {
      border-color: var(--text-secondary);
      color: var(--text-secondary);
    }

    .spinner {
      width: 20px;
      height: 20px;
      border: 2px solid rgba(var(--accent-primary), 0.2);
      border-top: 2px solid var(--accent-primary);
      border-radius: 50%;
      animation: spin 1s linear infinite;
      display: inline-block;
      margin-right: 0.5rem;
    }

    @keyframes spin {
      0% { transform: rotate(0deg); }
      100% { transform: rotate(360deg); }
    }

    @media (max-width: 768px) {
      .format-options {
        grid-template-columns: 1fr;
      }
      
      .result-stats {
        grid-template-columns: repeat(2, 1fr);
      }
    }
  `

  render() {
    return html`
      <div class="dialog" @click=${this._handleDialogClick}>
        <div class="dialog-header">
          <h2 class="dialog-title">📥 Import Bookmarks</h2>
          <button class="close-button" @click=${this._handleClose}>×</button>
        </div>

        ${this._error ? html`
          <div class="error-message">
            ⚠️ ${this._error}
          </div>
        ` : ''}

        ${this._result ? html`
          <div class="result-message">
            ✅ ${this._result.message}
            <div class="result-stats">
              <div class="result-stat">
                <div class="result-stat-number">${this._result.imported}</div>
                <div class="result-stat-label">Imported</div>
              </div>
              <div class="result-stat">
                <div class="result-stat-number">${this._result.skipped}</div>
                <div class="result-stat-label">Skipped</div>
              </div>
              <div class="result-stat">
                <div class="result-stat-number">${this._result.errors}</div>
                <div class="result-stat-label">Errors</div>
              </div>
            </div>
          </div>
        ` : ''}

        <div class="form-section">
          <h3 class="section-title">Choose Browser Format</h3>
          <div class="format-options">
            <label class="format-option ${this._selectedFormat === 'chrome' ? 'selected' : ''}">
              <input 
                type="radio" 
                class="format-radio" 
                name="format" 
                value="chrome"
                .checked=${this._selectedFormat === 'chrome'}
                @change=${this._handleFormatChange}>
              <div class="format-info">
                <div class="format-name">Chrome / Edge</div>
                <div class="format-description">Bookmarks JSON file</div>
              </div>
            </label>
            
            <label class="format-option ${this._selectedFormat === 'firefox' ? 'selected' : ''}">
              <input 
                type="radio" 
                class="format-radio" 
                name="format" 
                value="firefox"
                .checked=${this._selectedFormat === 'firefox'}
                @change=${this._handleFormatChange}>
              <div class="format-info">
                <div class="format-name">Firefox / Safari</div>
                <div class="format-description">HTML export file</div>
              </div>
            </label>
          </div>
        </div>

        <div class="instructions">
          <div class="instructions-title">How to export your bookmarks:</div>
          <div class="instructions-text">
            ${this._selectedFormat === 'chrome' ? html`
              <strong>Chrome/Edge:</strong>
              <ul>
                <li>Go to Settings → Bookmarks → Bookmark Manager</li>
                <li>Click the three dots menu → Export bookmarks</li>
                <li>Save the JSON file and upload it here</li>
              </ul>
            ` : html`
              <strong>Firefox/Safari:</strong>
              <ul>
                <li><strong>Firefox:</strong> Library → Bookmarks → Show All → Import & Backup → Export HTML</li>
                <li><strong>Safari:</strong> File → Export Bookmarks</li>
                <li>Upload the HTML file here</li>
              </ul>
            `}
          </div>
        </div>

        <div class="form-section">
          <h3 class="section-title">Select Bookmarks File</h3>
          <div class="file-upload">
            <input 
              type="file" 
              class="file-input"
              accept=${this._selectedFormat === 'chrome' ? '.json' : '.html,.htm'}
              @change=${this._handleFileChange}>
            <div class="file-button ${this._fileName ? 'has-file' : ''}">
              <span>${this._fileName ? '✓' : '📁'}</span>
              <span>${this._fileName || 'Choose file to upload'}</span>
            </div>
          </div>
          ${this._fileName ? html`
            <div class="file-name">📄 ${this._fileName}</div>
          ` : ''}
        </div>

        <div class="dialog-actions">
          <button 
            type="button" 
            class="button button-secondary" 
            @click=${this._handleClose}
            ?disabled=${this._isImporting}>
            Cancel
          </button>
          <button 
            type="button" 
            class="button button-primary" 
            @click=${this._handleImport}
            ?disabled=${!this._fileContent || this._isImporting}>
            ${this._isImporting ? html`<span class="spinner"></span>Importing...` : 'Import Bookmarks'}
          </button>
        </div>
      </div>
    `
  }

  private _handleDialogClick(e: Event) {
    e.stopPropagation()
  }

  private _handleClose() {
    this.dispatchEvent(new CustomEvent('close'))
  }

  private _handleFormatChange(e: Event) {
    const radio = e.target as HTMLInputElement
    this._selectedFormat = radio.value
    // Clear file when format changes
    this._fileContent = ''
    this._fileName = ''
    this._error = null
    this._result = null
  }

  private _handleFileChange(e: Event) {
    const input = e.target as HTMLInputElement
    const file = input.files?.[0]
    
    if (!file) {
      this._fileContent = ''
      this._fileName = ''
      return
    }

    this._fileName = file.name
    this._error = null
    this._result = null

    const reader = new FileReader()
    reader.onload = (e) => {
      this._fileContent = e.target?.result as string || ''
    }
    reader.onerror = () => {
      this._error = 'Failed to read file'
      this._fileContent = ''
      this._fileName = ''
    }
    reader.readAsText(file)
  }

  private async _handleImport() {
    if (!this._fileContent) return

    this._isImporting = true
    this._error = null
    this._result = null

    try {
      const response = await fetch('/api/bookmarks/import', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          format: this._selectedFormat,
          data: this._fileContent
        })
      })

      if (!response.ok) {
        throw new Error(`Import failed: ${response.statusText}`)
      }

      this._result = await response.json()
      
      // Dispatch success event to refresh bookmark list
      this.dispatchEvent(new CustomEvent('import-success', {
        detail: this._result
      }))

    } catch (error) {
      this._error = error instanceof Error ? error.message : 'Import failed'
    } finally {
      this._isImporting = false
    }
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'import-dialog': ImportDialog
  }
}