export interface InstanceInfo {
  id: string
  name: string
  createdAt: string
  jarPath: string
  javaPath: string
  dataDir: string
  isolate: boolean
  memoryMb: number
  jvmArgs: string[]
  gameArgs: string[]
  running: boolean
}

export interface InstanceList {
  items: InstanceInfo[]
  skipped: number
}

export interface LaunchLogPayload {
  id: string
  line: string
  stream: string
}

export interface LaunchExitPayload {
  id: string
  code: number
}
