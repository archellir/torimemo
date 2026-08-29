// Common type definitions for the application

export interface User {
  id: number
  username: string
  email: string
  full_name?: string
  is_admin: boolean
  created_at: string
  updated_at: string
  last_login_at?: string
}

export interface Bookmark {
  id: number
  title: string
  url: string
  description?: string
  favicon_url?: string
  created_at: string
  updated_at: string
  is_favorite: boolean
  tags?: Tag[]
}

export interface Tag {
  id: number
  name: string
  color?: string
  created_at: string
}

export interface Folder {
  id: number
  name: string
  description?: string
  color?: string
  icon?: string
  parent_id?: number
  sort_order: number
  created_at: string
  updated_at: string
  children?: Folder[]
}

export interface BookmarkListResponse {
  bookmarks: Bookmark[]
  total: number
  page: number
  limit: number
  has_more: boolean
  total_pages: number
  tag_count: number
  favorite_count: number
}

export interface SearchResult extends Bookmark {
  rank: number
  snippet?: string
}

// Authentication types
export interface LoginRequest {
  username: string
  password: string
}

export interface RegisterRequest {
  username: string
  email: string
  password: string
  full_name?: string
}

export interface AuthResponse {
  token: string
  expires_at: string
  user: User
}

export interface CreateBookmarkRequest {
  title: string
  url: string
  description?: string
  tags?: string[]
}

export interface UpdateBookmarkRequest {
  title?: string
  url?: string
  description?: string
  is_favorite?: boolean
  tags?: string[]
}

export interface CreateFolderRequest {
  name: string
  description?: string
  color?: string
  icon?: string
  parent_id?: number
  sort_order?: number
}

export interface UpdateFolderRequest {
  name?: string
  description?: string
  color?: string
  icon?: string
  parent_id?: number
  sort_order?: number
}

// Health check types
export interface BookmarkHealth {
  id: number
  url: string
  status: 'healthy' | 'broken' | 'slow' | 'redirect' | 'unknown'
  status_code: number
  response_time_ms: number
  redirect_url?: string
  error?: string
  last_checked: string
}

// Archive service types
export interface ArchivedContent {
  id: number
  url: string
  title: string
  content: string
  textContent: string
  screenshot?: string
  cached_at: number
  size: number
  status: 'cached' | 'failed' | 'pending'
}

export interface ArchiveStats {
  total_items: number
  total_size: number
  cache_hit_ratio: number
  oldest_entry: number
  newest_entry: number
}

// Theme types
export type ThemeMode = 'auto' | 'light' | 'dark'

// API response types
export interface ApiError {
  error: string
  status: number
}

// Event types
export interface SearchEvent {
  query: string
}

export interface AuthEvent {
  user: User
  token: string
}

export interface BookmarkEvent {
  bookmark: Bookmark
}

export interface TagEvent {
  tag: Tag
}

export interface FolderEvent {
  folder: Folder
}