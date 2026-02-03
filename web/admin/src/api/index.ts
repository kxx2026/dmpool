// API Client for DMPool Admin
import axios, { type AxiosInstance, type AxiosError } from 'axios'
import type {
  ApiResponse,
  LoginRequest,
  LoginResponse,
  DashboardMetrics,
  ConfigView,
  ConfigUpdate,
  PaginatedResponse,
  WorkerInfo,
  WorkerPaginationParams,
  AuditLog,
  AuditStats,
  AuditFilter,
  BackupMetadata,
  BackupStats,
  AlertRule,
  Alert,
  AlertStats,
  HealthStatus
} from '@/types'

class ApiError extends Error {
  constructor(
    message: string,
    public statusCode?: number,
    public responseData?: any
  ) {
    super(message)
    this.name = 'ApiError'
  }
}

class ApiClient {
  private client: AxiosInstance
  private token: string | null = null

  constructor() {
    this.client = axios.create({
      baseURL: '/api',
      timeout: 30000,
      headers: {
        'Content-Type': 'application/json'
      }
    })

    // Request interceptor
    this.client.interceptors.request.use(
      (config) => {
        if (this.token) {
          config.headers.Authorization = `Bearer ${this.token}`
        }
        return config
      },
      (error) => Promise.reject(error)
    )

    // Response interceptor
    this.client.interceptors.response.use(
      (response) => response,
      (error: AxiosError<any>) => {
        const message = error.response?.data?.message || error.message || 'Request failed'
        throw new ApiError(message, error.response?.status, error.response?.data)
      }
    )

    // Load token from localStorage
    this.loadToken()
  }

  private loadToken() {
    this.token = localStorage.getItem('auth_token')
  }

  private saveToken(token: string) {
    this.token = token
    localStorage.setItem('auth_token', token)
  }

  private clearToken() {
    this.token = null
    localStorage.removeItem('auth_token')
  }

  isAuthenticated(): boolean {
    return !!this.token
  }

  async login(credentials: LoginRequest): Promise<LoginResponse> {
    const response = await this.client.post<ApiResponse<LoginResponse>>('/auth/login', credentials)
    const { token, user } = response.data.data!
    this.saveToken(token)
    return { token, user }
  }

  logout() {
    this.clearToken()
  }

  // Dashboard
  async getDashboard(): Promise<DashboardMetrics> {
    const response = await this.client.get<ApiResponse<DashboardMetrics>>('/dashboard')
    return response.data.data!
  }

  // Configuration
  async getConfig(): Promise<ConfigView> {
    const response = await this.client.get<ApiResponse<ConfigView>>('/config')
    return response.data.data!
  }

  async updateConfig(update: ConfigUpdate): Promise<any> {
    const response = await this.client.post<ApiResponse>('/config', update)
    return response.data.data
  }

  async reloadConfig(): Promise<any> {
    const response = await this.client.post<ApiResponse>('/config/reload')
    return response.data.data
  }

  // Workers
  async getWorkers(params?: WorkerPaginationParams): Promise<PaginatedResponse<WorkerInfo>> {
    const response = await this.client.get<ApiResponse<PaginatedResponse<WorkerInfo>>>(
      '/workers',
      { params }
    )
    return response.data.data!
  }

  async getWorker(address: string): Promise<WorkerInfo> {
    const response = await this.client.get<ApiResponse<WorkerInfo>>(`/workers/${address}`)
    return response.data.data!
  }

  async banWorker(address: string, reason?: string): Promise<any> {
    const response = await this.client.post<ApiResponse>(`/workers/${address}/ban`, { reason })
    return response.data.data
  }

  async unbanWorker(address: string): Promise<any> {
    const response = await this.client.post<ApiResponse>(`/workers/${address}/unban`)
    return response.data.data
  }

  async addWorkerTag(address: string, tag: string): Promise<any> {
    const response = await this.client.post<ApiResponse>(`/workers/${address}/tags`, { tag })
    return response.data.data
  }

  async removeWorkerTag(address: string, tag: string): Promise<any> {
    const response = await this.client.post<ApiResponse>(`/workers/${address}/tags/${tag}`)
    return response.data.data
  }

  // Audit
  async getAuditLogs(filter?: AuditFilter): Promise<AuditLog[]> {
    const response = await this.client.get<ApiResponse<AuditLog[]>>('/audit/logs', {
      params: filter
    })
    return response.data.data!
  }

  async getAuditStats(): Promise<AuditStats> {
    const response = await this.client.get<ApiResponse<AuditStats>>('/audit/stats')
    return response.data.data!
  }

  // Backup
  async createBackup(): Promise<BackupMetadata> {
    const response = await this.client.post<ApiResponse<{ backup: BackupMetadata }>>('/backup/create')
    return response.data.data!.backup
  }

  async listBackups(): Promise<{ backups: BackupMetadata[]; count: number }> {
    const response = await this.client.get<ApiResponse<{ backups: BackupMetadata[]; count: number }>>(
      '/backup/list'
    )
    return response.data.data!
  }

  async getBackupStats(): Promise<BackupStats> {
    const response = await this.client.get<ApiResponse<{ stats: BackupStats }>>('/backup/stats')
    return response.data.data!.stats
  }

  async deleteBackup(id: string): Promise<any> {
    const response = await this.client.post<ApiResponse>(`/backup/${id}/delete`)
    return response.data.data
  }

  async restoreBackup(id: string): Promise<any> {
    const response = await this.client.post<ApiResponse>(`/backup/${id}/restore`)
    return response.data.data
  }

  async cleanupBackups(): Promise<any> {
    const response = await this.client.post<ApiResponse>('/backup/cleanup')
    return response.data.data
  }

  // Health
  async getHealth(): Promise<HealthStatus> {
    const response = await this.client.get<any>('/health')
    return response.data
  }

  async getServicesStatus(): Promise<any> {
    const response = await this.client.get<any>('/services/status')
    return response.data
  }
}

export const api = new ApiClient()
export { ApiError }
export type { ApiClient }
