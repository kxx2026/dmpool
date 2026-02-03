// API Types for DMPool Admin

export interface ApiResponse<T = any> {
  status: 'ok' | 'error'
  data?: T
  message?: string
  timestamp: number
}

// Auth
export interface LoginRequest {
  username: string
  password: string
}

export interface LoginResponse {
  token: string
  user: UserInfo
}

export interface UserInfo {
  username: string
  role: 'admin' | 'user'
}

// Dashboard
export interface DashboardMetrics {
  pool_hashrate_ths: number
  active_workers: number
  total_shares: number
  blocks_found: number
  uptime_seconds: number
  pplns_window_shares: number
  current_difficulty: number
}

// Configuration
export interface ConfigView {
  stratum_port: number
  stratum_hostname: string
  start_difficulty: number
  minimum_difficulty: number
  pplns_ttl_days: number
  difficulty_multiplier: number
  donation: number
  ignore_difficulty: boolean
  pool_signature: string | null
}

export interface ConfigUpdate {
  start_difficulty?: number
  minimum_difficulty?: number
  pplns_ttl_days?: number
  donation?: number
  ignore_difficulty?: boolean
  pool_signature?: string
}

// Workers
export interface WorkerInfo {
  address: string
  worker_name: string
  hashrate_ths: number
  shares_count: number
  difficulty: number
  last_seen: string
  first_seen: string
  is_banned: boolean
  tags: string[]
  status: 'active' | 'inactive' | 'banned'
}

export interface PaginatedResponse<T> {
  data: T[]
  total: number
  page: number
  page_size: number
  total_pages: number
}

export interface WorkerPaginationParams {
  page?: number
  page_size?: number
  search?: string
  status?: string
  sort_by?: string
  sort_order?: string
}

// Audit
export interface AuditLog {
  id: string
  timestamp: string
  username: string
  action: string
  resource: string
  ip_address: string
  details: Record<string, any>
  success: boolean
  error?: string
}

export interface AuditStats {
  total_logs: number
  success_count: number
  failure_count: number
  top_actions: [string, number][]
  oldest_log?: string
  newest_log?: string
}

export interface AuditFilter {
  username?: string
  action?: string
  resource?: string
  start_time?: number
  end_time?: number
  limit?: number
}

// Backup
export interface BackupMetadata {
  id: string
  timestamp: string
  file_path: string
  original_size: number
  backup_size: number
  compression_ratio?: number
  validated: boolean
  schema_version: number
  checksum: string
}

export interface BackupStats {
  total_backups: number
  total_size_bytes: number
  latest_backup?: string
  oldest_backup?: string
  disk_usage_bytes: number
}

// Alerts
export enum AlertLevel {
  Info = 'info',
  Warning = 'warning',
  Critical = 'critical'
}

export interface AlertRule {
  id: string
  name: string
  description: string
  condition: any
  level: AlertLevel
  enabled: boolean
  channels: string[]
  cooldown_minutes: number
}

export interface Alert {
  id: string
  rule_id: string
  level: AlertLevel
  title: string
  message: string
  context: Record<string, any>
  triggered_at: string
  acknowledged: boolean
  channel: string
}

export interface AlertStats {
  total_alerts: number
  active_alerts: number
  acknowledged_alerts: number
  alerts_by_level: Record<string, number>
  alerts_by_rule: Record<string, number>
}

// Health
export interface HealthStatus {
  status: 'healthy' | 'degraded' | 'unhealthy'
  components: ComponentStatus[]
}

export interface ComponentStatus {
  name: string
  status: 'up' | 'down' | 'degraded'
  message?: string
  last_check: string
}
