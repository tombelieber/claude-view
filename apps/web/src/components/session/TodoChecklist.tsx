import { CheckCircle, Circle, ListTodo } from 'lucide-react'
import type { AgentTodos } from '../../types/generated/AgentTodos'
import type { TodoItem } from '../../types/generated/TodoItem'

function TodoItemRow({ item }: { item: TodoItem }) {
  const isCompleted = item.status === 'completed'
  return (
    <li className="flex items-start gap-2 py-0.5">
      {isCompleted ? (
        <CheckCircle className="w-3.5 h-3.5 text-green-500 dark:text-green-400 shrink-0 mt-0.5" />
      ) : (
        <Circle className="w-3.5 h-3.5 text-gray-400 dark:text-gray-500 shrink-0 mt-0.5" />
      )}
      <span
        className={`text-xs ${
          isCompleted
            ? 'text-gray-400 dark:text-gray-500 line-through'
            : 'text-gray-700 dark:text-gray-300'
        }`}
      >
        {item.content}
      </span>
    </li>
  )
}

function AgentTodoSection({ agentTodo }: { agentTodo: AgentTodos }) {
  const completed = agentTodo.items.filter((i) => i.status === 'completed').length
  const total = agentTodo.items.length

  return (
    <div>
      {!agentTodo.isMainAgent && (
        <span className="text-xs font-medium text-gray-400 dark:text-gray-500 mb-1 block">
          Subagent {agentTodo.agentId.slice(0, 8)}…
        </span>
      )}
      <div className="flex items-center gap-1.5 mb-1">
        <span className="text-xs font-medium text-gray-500 dark:text-gray-400">
          {completed}/{total} done
        </span>
        <div className="flex-1 h-1 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden max-w-20">
          <div
            className="h-full bg-green-500 dark:bg-green-400 rounded-full transition-all"
            style={{ width: `${(completed / total) * 100}%` }}
          />
        </div>
      </div>
      <ul className="space-y-0.5">
        {agentTodo.items.map((item, i) => (
          <TodoItemRow key={i} item={item} />
        ))}
      </ul>
    </div>
  )
}

interface TodoChecklistProps {
  todos: AgentTodos[]
}

/** Render agent-level todo checklists for a session detail view. */
export function TodoChecklist({ todos }: TodoChecklistProps) {
  if (!todos || todos.length === 0) return null

  return (
    <div className="border border-gray-200 dark:border-gray-800 rounded-lg p-3 bg-white dark:bg-gray-900/50">
      <div className="flex items-center gap-2 mb-2">
        <ListTodo className="w-4 h-4 text-apple-text3" />
        <h3 className="text-sm font-semibold text-apple-text1 tracking-tight">Agent Todos</h3>
      </div>
      <div className="space-y-3">
        {todos.map((agentTodo) => (
          <AgentTodoSection key={agentTodo.agentId} agentTodo={agentTodo} />
        ))}
      </div>
    </div>
  )
}
