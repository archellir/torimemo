import { LitElement, html, css } from 'lit'
import { customElement, state } from 'lit/decorators.js'

export interface ExportData {
  version: string
  exported_at: string
  bookmarks: any[]
  tags: any[]
}

@customElement('export-dialog')
export class ExportDialog extends LitElement {
  @state() private _selectedFormat = 'json'
  @state() private _isExporting = false
  @state() private _error: string | null = null

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
      max-width: 500px;
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

    .format-options {
      display: grid;
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

    .export-info {
      background: rgba(var(--accent-secondary), 0.05);
      border: 1px solid rgba(var(--accent-secondary), 0.2);
      border-radius: 0.5rem;
      padding: 1rem;
      margin-bottom: 1.5rem;
    }

    .export-info-title {
      color: var(--accent-secondary);
      font-weight: bold;
      margin-bottom: 0.5rem;
    }

    .export-info-text {
      color: var(--text-secondary);
      font-size: 0.875rem;
      line-height: 1.4;
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
  `

  render() {
    return html`
      <div class="dialog" @click=${this._handleDialogClick}>
        <div class="dialog-header">
          <h2 class="dialog-title">📤 Export Bookmarks</h2>
          <button class="close-button" @click=${this._handleClose}>×</button>
        </div>

        ${this._error ? html`
          <div class="error-message">
            ⚠️ ${this._error}
          </div>
        ` : ''}

        <div class="format-options">
          <label class="format-option ${this._selectedFormat === 'json' ? 'selected' : ''}">
            <input 
              type="radio" 
              class="format-radio" 
              name="format" 
              value="json"
              .checked=${this._selectedFormat === 'json'}
              @change=${this._handleFormatChange}>
            <div class="format-info">
              <div class="format-name">Torimemo JSON</div>
              <div class="format-description">Native format with all metadata</div>
            </div>
          </label>
          
          <label class="format-option ${this._selectedFormat === 'html' ? 'selected' : ''}">
            <input 
              type="radio" 
              class="format-radio" 
              name="format" 
              value="html"
              .checked=${this._selectedFormat === 'html'}
              @change=${this._handleFormatChange}>
            <div class="format-info">
              <div class="format-name">HTML Export</div>
              <div class="format-description">Universal format for browser import</div>
            </div>
          </label>
        </div>

        <div class="export-info">
          <div class="export-info-title">Export Information</div>
          <div class="export-info-text">
            ${this._selectedFormat === 'json' ? 
              'JSON export includes all bookmarks, tags, descriptions, and metadata. This format can be reimported to Torimemo.' :
              'HTML export creates a browser-compatible bookmarks file that can be imported into Chrome, Firefox, Safari, or other browsers.'
            }
          </div>
        </div>

        <div class="dialog-actions">
          <button 
            type="button" 
            class="button button-secondary" 
            @click=${this._handleClose}
            ?disabled=${this._isExporting}>
            Cancel
          </button>
          <button 
            type="button" 
            class="button button-primary" 
            @click=${this._handleExport}
            ?disabled=${this._isExporting}>
            ${this._isExporting ? html`<span class="spinner"></span>Exporting...` : 'Export Bookmarks'}
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
    this._error = null
  }

  private async _handleExport() {
    this._isExporting = true
    this._error = null

    try {
      const url = this._selectedFormat === 'json' ? '/api/export' : '/api/export?format=html'
      
      const response = await fetch(url, {
        method: 'GET'
      })

      if (!response.ok) {
        throw new Error(`Export failed: ${response.statusText}`)
      }

      // Get filename from Content-Disposition header or create default
      const contentDisposition = response.headers.get('content-disposition')
      let filename = `torimemo-export-${new Date().toISOString().split('T')[0]}`
      
      if (contentDisposition) {
        const match = contentDisposition.match(/filename[^;=\n]*=((['"]).*?\2|[^;\n]*)/)
        if (match?.[1]) {
          filename = match[1].replace(/['"]/g, '')
        }
      } else {
        filename += this._selectedFormat === 'json' ? '.json' : '.html'
      }

      // Download the file
      const blob = await response.blob()
      const downloadUrl = URL.createObjectURL(blob)
      
      const link = document.createElement('a')
      link.href = downloadUrl
      link.download = filename
      document.body.appendChild(link)
      link.click()
      document.body.removeChild(link)
      
      URL.revokeObjectURL(downloadUrl)

      // Close dialog on success
      this._handleClose()

    } catch (error) {
      this._error = error instanceof Error ? error.message : 'Export failed'
    } finally {
      this._isExporting = false
    }
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'export-dialog': ExportDialog
  }
}