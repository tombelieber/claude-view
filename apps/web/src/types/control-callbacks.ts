export interface ControlCallbacks {
  answerQuestion: (requestId: string, answers: Record<string, string>) => void
  respondPermission: (requestId: string, allowed: boolean) => void
  approvePlan: (requestId: string, approved: boolean, feedback?: string) => void
  submitElicitation: (requestId: string, response: string) => void
}
