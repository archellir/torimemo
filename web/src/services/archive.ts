import type { ArchivedContent, ArchiveStats } from '~/types'

// Re-export for backward compatibility
export type { ArchivedContent, ArchiveStats }

export class ArchiveService {
  private dbName = 'torimemo-archive'
  private dbVersion = 1
  private db: IDBDatabase | null = null
  private maxCacheSize = 100 * 1024 * 1024 // 100MB
  private maxEntries = 1000

  constructor() {
    this.initDB()
  }

  // Initialize IndexedDB for offline storage
  private async initDB(): Promise<void> {
    return new Promise((resolve, reject) => {
      const request = indexedDB.open(this.dbName, this.dbVersion)

      request.onerror = () => reject(request.error)
      request.onsuccess = () => {
        this.db = request.result
        resolve()
      }

      request.onupgradeneeded = (event) => {
        const db = (event.target as IDBOpenDBRequest).result

        // Create archive store
        if (!db.objectStoreNames.contains('archive')) {
          const store = db.createObjectStore('archive', { keyPath: 'id' })
          store.createIndex('url', 'url', { unique: true })
          store.createIndex('cached_at', 'cached_at', { unique: false })
          store.createIndex('size', 'size', { unique: false })
        }

        // Create metadata store
        if (!db.objectStoreNames.contains('metadata')) {
          db.createObjectStore('metadata', { keyPath: 'key' })
        }
      }
    })
  }

  // Archive a bookmark's content
  async archiveBookmark(bookmark: any): Promise<ArchivedContent | null> {
    if (!this.db) {
      await this.initDB()
    }

    try {
      // Check if already cached
      const existing = await this.getCachedContent(bookmark.id)
      if (existing && existing.status === 'cached') {
        return existing
      }

      // Fetch content from our backend
      const response = await fetch('/api/archive/content', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json'
        },
        body: JSON.stringify({
          url: bookmark.url,
          bookmark_id: bookmark.id
        })
      })

      if (!response.ok) {
        throw new Error(`Archive request failed: ${response.statusText}`)
      }

      const archiveData = await response.json()

      // Store in IndexedDB
      const archivedContent: ArchivedContent = {
        id: bookmark.id,
        url: bookmark.url,
        title: archiveData.title || bookmark.title,
        content: archiveData.content || '',
        textContent: archiveData.text_content || '',
        screenshot: archiveData.screenshot || undefined,
        cached_at: Date.now(),
        size: new Blob([archiveData.content || '']).size,
        status: 'cached'
      }

      await this.storeContent(archivedContent)
      await this.cleanupOldEntries()

      return archivedContent

    } catch (error) {
      console.error('Failed to archive bookmark:', error)
      
      // Store failed attempt
      const failedContent: ArchivedContent = {
        id: bookmark.id,
        url: bookmark.url,
        title: bookmark.title,
        content: '',
        textContent: '',
        cached_at: Date.now(),
        size: 0,
        status: 'failed'
      }

      await this.storeContent(failedContent)
      return failedContent
    }
  }

  // Get cached content for a bookmark
  async getCachedContent(bookmarkId: number): Promise<ArchivedContent | null> {
    if (!this.db) {
      await this.initDB()
    }

    return new Promise((resolve, reject) => {
      const transaction = this.db!.transaction(['archive'], 'readonly')
      const store = transaction.objectStore('archive')
      const request = store.get(bookmarkId)

      request.onsuccess = () => resolve(request.result || null)
      request.onerror = () => reject(request.error)
    })
  }

  // Store content in IndexedDB
  private async storeContent(content: ArchivedContent): Promise<void> {
    return new Promise((resolve, reject) => {
      const transaction = this.db!.transaction(['archive'], 'readwrite')
      const store = transaction.objectStore('archive')
      const request = store.put(content)

      request.onsuccess = () => resolve()
      request.onerror = () => reject(request.error)
    })
  }

  // Get all cached bookmarks
  async getAllCached(): Promise<ArchivedContent[]> {
    if (!this.db) {
      await this.initDB()
    }

    return new Promise((resolve, reject) => {
      const transaction = this.db!.transaction(['archive'], 'readonly')
      const store = transaction.objectStore('archive')
      const request = store.getAll()

      request.onsuccess = () => resolve(request.result || [])
      request.onerror = () => reject(request.error)
    })
  }

  // Search cached content
  async searchCached(query: string): Promise<ArchivedContent[]> {
    const allCached = await this.getAllCached()
    const normalizedQuery = query.toLowerCase()

    return allCached.filter(item => 
      item.status === 'cached' && (
        item.title.toLowerCase().includes(normalizedQuery) ||
        item.textContent.toLowerCase().includes(normalizedQuery) ||
        item.url.toLowerCase().includes(normalizedQuery)
      )
    ).sort((a, b) => b.cached_at - a.cached_at)
  }

  // Get archive statistics
  async getStats(): Promise<ArchiveStats> {
    const allCached = await this.getAllCached()
    const cached = allCached.filter(item => item.status === 'cached')
    
    const totalSize = cached.reduce((sum, item) => sum + item.size, 0)
    const timestamps = cached.map(item => item.cached_at)
    
    return {
      total_items: cached.length,
      total_size: totalSize,
      cache_hit_ratio: cached.length / Math.max(allCached.length, 1),
      oldest_entry: Math.min(...timestamps) || 0,
      newest_entry: Math.max(...timestamps) || 0
    }
  }

  // Clear all cached content
  async clearCache(): Promise<void> {
    if (!this.db) {
      await this.initDB()
    }

    return new Promise((resolve, reject) => {
      const transaction = this.db!.transaction(['archive'], 'readwrite')
      const store = transaction.objectStore('archive')
      const request = store.clear()

      request.onsuccess = () => resolve()
      request.onerror = () => reject(request.error)
    })
  }

  // Remove specific cached item
  async removeCached(bookmarkId: number): Promise<void> {
    if (!this.db) {
      await this.initDB()
    }

    return new Promise((resolve, reject) => {
      const transaction = this.db!.transaction(['archive'], 'readwrite')
      const store = transaction.objectStore('archive')
      const request = store.delete(bookmarkId)

      request.onsuccess = () => resolve()
      request.onerror = () => reject(request.error)
    })
  }

  // Cleanup old entries to stay within limits
  private async cleanupOldEntries(): Promise<void> {
    const allCached = await this.getAllCached()
    const cached = allCached.filter(item => item.status === 'cached')
    
    // Check entry count limit
    if (cached.length > this.maxEntries) {
      cached.sort((a, b) => a.cached_at - b.cached_at)
      const toRemove = cached.slice(0, cached.length - this.maxEntries)
      
      for (const item of toRemove) {
        await this.removeCached(item.id)
      }
    }

    // Check size limit
    const totalSize = cached.reduce((sum, item) => sum + item.size, 0)
    if (totalSize > this.maxCacheSize) {
      cached.sort((a, b) => a.cached_at - b.cached_at)
      
      let removedSize = 0
      for (const item of cached) {
        await this.removeCached(item.id)
        removedSize += item.size
        
        if (totalSize - removedSize < this.maxCacheSize * 0.8) { // Remove to 80% capacity
          break
        }
      }
    }
  }

  // Check if offline (no network connection)
  isOffline(): boolean {
    return !navigator.onLine
  }

  // Preload/prefetch content for bookmarks
  async preloadBookmarks(bookmarks: any[]): Promise<void> {
    const batchSize = 5 // Process 5 at a time to avoid overwhelming
    
    for (let i = 0; i < bookmarks.length; i += batchSize) {
      const batch = bookmarks.slice(i, i + batchSize)
      
      const promises = batch.map(async bookmark => {
        const existing = await this.getCachedContent(bookmark.id)
        if (!existing || existing.status === 'failed') {
          return this.archiveBookmark(bookmark)
        }
      })
      
      await Promise.allSettled(promises)
      
      // Small delay between batches
      if (i + batchSize < bookmarks.length) {
        await new Promise(resolve => setTimeout(resolve, 1000))
      }
    }
  }

  // Export cached content for backup
  async exportCache(): Promise<Blob> {
    const allCached = await this.getAllCached()
    const exportData = {
      export_date: new Date().toISOString(),
      version: '1.0',
      cache: allCached
    }
    
    return new Blob([JSON.stringify(exportData, null, 2)], {
      type: 'application/json'
    })
  }

  // Import cached content from backup
  async importCache(file: File): Promise<void> {
    const text = await file.text()
    const data = JSON.parse(text)
    
    if (!data.cache || !Array.isArray(data.cache)) {
      throw new Error('Invalid cache export file')
    }
    
    for (const item of data.cache) {
      await this.storeContent(item)
    }
  }
}

// Export singleton instance
export const archiveService = new ArchiveService()