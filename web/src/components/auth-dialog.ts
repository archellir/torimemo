import { LitElement, html, css } from 'lit'
import { customElement, state } from 'lit/decorators.js'
import type { LoginRequest, RegisterRequest, AuthResponse } from '~/types'

@customElement('auth-dialog')
export class AuthDialog extends LitElement {
  @state() private _isOpen = false
  @state() private _mode: 'login' | 'register' = 'login'
  @state() private _loading = false
  @state() private _error = ''
  @state() private _formData = {
    username: '',
    email: '',
    password: '',
    confirmPassword: '',
    fullName: ''
  }

  static styles = css`
    :host {
      position: fixed;
      top: 0;
      left: 0;
      right: 0;
      bottom: 0;
      z-index: 1000;
      pointer-events: none;
    }

    :host([open]) {
      pointer-events: all;
    }

    .overlay {
      transition: opacity 0.3s ease;
    }

    .dialog {
      transition: all 0.3s ease;
      transform: scale(0.95) translateY(20px);
      opacity: 0;
    }

    :host([open]) .dialog {
      transform: scale(1) translateY(0);
      opacity: 1;
    }
  `

  open() {
    this._isOpen = true
    this.setAttribute('open', '')
    this._resetForm()
  }

  close() {
    this._isOpen = false
    this.removeAttribute('open')
    this._error = ''
  }

  private _resetForm() {
    this._formData = {
      username: '',
      email: '',
      password: '',
      confirmPassword: '',
      fullName: ''
    }
    this._error = ''
  }

  private _switchMode() {
    this._mode = this._mode === 'login' ? 'register' : 'login'
    this._resetForm()
  }

  private _handleInput(field: string, value: string) {
    this._formData = {
      ...this._formData,
      [field]: value
    }
  }

  private async _handleSubmit(e: Event) {
    e.preventDefault()
    if (this._loading) return

    this._error = ''
    this._loading = true

    try {
      if (this._mode === 'login') {
        await this._login()
      } else {
        await this._register()
      }
    } catch (error) {
      this._error = error instanceof Error ? error.message : 'An error occurred'
    } finally {
      this._loading = false
    }
  }

  private async _login() {
    const loginData: LoginRequest = {
      username: this._formData.username,
      password: this._formData.password
    }

    const response = await fetch('/api/auth/login', {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json'
      },
      body: JSON.stringify(loginData)
    })

    if (!response.ok) {
      const error = await response.json()
      throw new Error(error.error || 'Login failed')
    }

    const authResponse: AuthResponse = await response.json()
    
    // Store token and user info
    localStorage.setItem('auth_token', authResponse.token)
    localStorage.setItem('user', JSON.stringify(authResponse.user))

    // Dispatch success event
    this.dispatchEvent(new CustomEvent('auth-success', {
      detail: { user: authResponse.user, token: authResponse.token }
    }))

    this.close()
  }

  private async _register() {
    if (this._formData.password !== this._formData.confirmPassword) {
      throw new Error('Passwords do not match')
    }

    const registerData: RegisterRequest = {
      username: this._formData.username,
      email: this._formData.email,
      password: this._formData.password,
      full_name: this._formData.fullName || undefined
    }

    const response = await fetch('/api/auth/register', {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json'
      },
      body: JSON.stringify(registerData)
    })

    if (!response.ok) {
      const error = await response.json()
      throw new Error(error.error || 'Registration failed')
    }

    const authResponse: AuthResponse = await response.json()
    
    // Store token and user info
    localStorage.setItem('auth_token', authResponse.token)
    localStorage.setItem('user', JSON.stringify(authResponse.user))

    // Dispatch success event
    this.dispatchEvent(new CustomEvent('auth-success', {
      detail: { user: authResponse.user, token: authResponse.token }
    }))

    this.close()
  }

  render() {
    if (!this._isOpen) return html``

    return html`
      <div class="cyber-modal">
        <div class="overlay absolute inset-0 bg-black/80 backdrop-blur-sm"
             @click=${this.close}></div>
        
        <div class="dialog cyber-modal-content max-w-md w-full mx-4">
          <!-- Header -->
          <div class="flex-between mb-6">
            <h2 class="text-2xl font-bold neon-cyan">
              ${this._mode === 'login' ? 'Login' : 'Create Account'}
            </h2>
            <button @click=${this.close} 
                    class="text-gray-400 hover:text-white transition-colors">
              <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" 
                      d="M6 18L18 6M6 6l12 12"></path>
              </svg>
            </button>
          </div>

          <!-- Error Display -->
          ${this._error ? html`
            <div class="bg-red-900/20 border border-red-500/50 rounded-lg p-3 mb-4">
              <div class="flex items-center">
                <svg class="w-5 h-5 text-red-400 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" 
                        d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"></path>
                </svg>
                <span class="text-red-400 text-sm">${this._error}</span>
              </div>
            </div>
          ` : ''}

          <!-- Form -->
          <form @submit=${this._handleSubmit} class="space-y-4">
            <!-- Username -->
            <div>
              <label class="block text-sm font-medium text-gray-300 mb-2">
                Username
              </label>
              <input type="text" 
                     .value=${this._formData.username}
                     @input=${(e: Event) => this._handleInput('username', (e.target as HTMLInputElement).value)}
                     class="cyber-input w-full"
                     placeholder="Enter your username"
                     required>
            </div>

            <!-- Email (Register only) -->
            ${this._mode === 'register' ? html`
              <div>
                <label class="block text-sm font-medium text-gray-300 mb-2">
                  Email
                </label>
                <input type="email" 
                       .value=${this._formData.email}
                       @input=${(e: Event) => this._handleInput('email', (e.target as HTMLInputElement).value)}
                       class="cyber-input w-full"
                       placeholder="Enter your email"
                       required>
              </div>

              <div>
                <label class="block text-sm font-medium text-gray-300 mb-2">
                  Full Name (Optional)
                </label>
                <input type="text" 
                       .value=${this._formData.fullName}
                       @input=${(e: Event) => this._handleInput('fullName', (e.target as HTMLInputElement).value)}
                       class="cyber-input w-full"
                       placeholder="Enter your full name">
              </div>
            ` : ''}

            <!-- Password -->
            <div>
              <label class="block text-sm font-medium text-gray-300 mb-2">
                Password
              </label>
              <input type="password" 
                     .value=${this._formData.password}
                     @input=${(e: Event) => this._handleInput('password', (e.target as HTMLInputElement).value)}
                     class="cyber-input w-full"
                     placeholder="Enter your password"
                     required>
            </div>

            <!-- Confirm Password (Register only) -->
            ${this._mode === 'register' ? html`
              <div>
                <label class="block text-sm font-medium text-gray-300 mb-2">
                  Confirm Password
                </label>
                <input type="password" 
                       .value=${this._formData.confirmPassword}
                       @input=${(e: Event) => this._handleInput('confirmPassword', (e.target as HTMLInputElement).value)}
                       class="cyber-input w-full"
                       placeholder="Confirm your password"
                       required>
              </div>
            ` : ''}

            <!-- Submit Button -->
            <button type="submit" 
                    ?disabled=${this._loading}
                    class="cyber-btn w-full relative overflow-hidden">
              ${this._loading ? html`
                <div class="flex items-center justify-center">
                  <div class="cyber-spinner mr-2"></div>
                  Processing...
                </div>
              ` : html`
                ${this._mode === 'login' ? 'Login' : 'Create Account'}
              `}
            </button>
          </form>

          <!-- Mode Switch -->
          <div class="mt-6 text-center">
            <button type="button" 
                    @click=${this._switchMode}
                    class="text-cyan-400 hover:text-cyan-300 transition-colors text-sm">
              ${this._mode === 'login' 
                ? "Don't have an account? Sign up" 
                : "Already have an account? Login"}
            </button>
          </div>

          <!-- Demo Info -->
          ${this._mode === 'login' ? html`
            <div class="mt-4 p-3 bg-yellow-900/20 border border-yellow-500/30 rounded-lg">
              <div class="text-xs text-yellow-300">
                <strong>Demo Account:</strong><br>
                Username: <code class="text-yellow-200">admin</code><br>
                Password: <code class="text-yellow-200">admin123</code>
              </div>
            </div>
          ` : ''}
        </div>
      </div>
    `
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'auth-dialog': AuthDialog
  }
}