import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import { XmlCard, detectXmlType, extractXmlBlocks } from './XmlCard'

// Mock ToolCallCard to verify it gets rendered with correct props
vi.mock('./ToolCallCard', () => ({
  ToolCallCard: ({ name, input, description }: { name: string; input: Record<string, unknown>; description: string }) => (
    <div data-testid="tool-call-card" data-name={name} data-description={description}>
      ToolCallCard: {name} - {description} - {JSON.stringify(input)}
    </div>
  ),
}))

// Mock StructuredDataCard to verify it gets rendered with correct props
vi.mock('./StructuredDataCard', () => ({
  StructuredDataCard: ({ xml, type }: { xml: string; type?: string }) => (
    <div data-testid="structured-data-card" data-type={type}>
      StructuredDataCard: {xml}
    </div>
  ),
}))

describe('XmlCard', () => {
  describe('tool_call type renders ToolCallCard', () => {
    it('renders ToolCallCard instead of CodeBlock for tool_call type', () => {
      const xml = `<tool_call>
        <tool_name>Read</tool_name>
        <parameters>{"file_path": "/src/index.ts"}</parameters>
        <what_happened>Read a file</what_happened>
      </tool_call>`

      render(<XmlCard content={xml} type="tool_call" />)

      expect(screen.getByTestId('tool-call-card')).toBeInTheDocument()
      expect(screen.getByTestId('tool-call-card')).toHaveAttribute('data-name', 'Read')
      expect(screen.getByTestId('tool-call-card')).toHaveAttribute('data-description', 'Read a file')
    })

    it('extracts JSON input from parameters tag', () => {
      const xml = `<tool_call>
        <tool_name>Bash</tool_name>
        <parameters>{"command": "npm install"}</parameters>
        <what_happened>Run install</what_happened>
      </tool_call>`

      render(<XmlCard content={xml} type="tool_call" />)

      const card = screen.getByTestId('tool-call-card')
      expect(card.textContent).toContain('"command":"npm install"')
    })

    it('extracts working_directory as input field', () => {
      const xml = `<tool_call>
        <tool_name>Bash</tool_name>
        <parameters>{"command": "ls"}</parameters>
        <what_happened>List files</what_happened>
        <working_directory>/Users/dev/project</working_directory>
      </tool_call>`

      render(<XmlCard content={xml} type="tool_call" />)

      const card = screen.getByTestId('tool-call-card')
      expect(card.textContent).toContain('/Users/dev/project')
    })

    it('falls back to "Tool Call" name when no name tag', () => {
      const xml = `<tool_call>
        <parameters>{"file_path": "/foo.ts"}</parameters>
        <what_happened>Did something</what_happened>
      </tool_call>`

      render(<XmlCard content={xml} type="tool_call" />)

      expect(screen.getByTestId('tool-call-card')).toHaveAttribute('data-name', 'Tool Call')
    })

    it('handles non-JSON parameters gracefully', () => {
      const xml = `<tool_call>
        <tool_name>Read</tool_name>
        <parameters>some plain text value</parameters>
        <what_happened>Read something</what_happened>
      </tool_call>`

      render(<XmlCard content={xml} type="tool_call" />)

      const card = screen.getByTestId('tool-call-card')
      expect(card.textContent).toContain('some plain text value')
    })

    it('does not render CodeBlock for tool_call type', () => {
      const xml = `<tool_call>
        <tool_name>Read</tool_name>
        <parameters>{"file_path": "/src/index.ts"}</parameters>
        <what_happened>Read a file</what_happened>
      </tool_call>`

      const { container } = render(<XmlCard content={xml} type="tool_call" />)

      // CodeBlock would render a <pre><code> block; ToolCallCard does not
      expect(container.querySelector('[data-testid="tool-call-card"]')).toBeInTheDocument()
      // No CodeBlock should be present
      expect(container.querySelector('code')).not.toBeInTheDocument()
    })
  })

  describe('unknown type renders StructuredDataCard', () => {
    it('renders StructuredDataCard instead of CodeBlock for unknown type', () => {
      const xml = `<custom_tag>
        <inner>Some structured data</inner>
      </custom_tag>`

      render(<XmlCard content={xml} type="unknown" />)

      expect(screen.getByTestId('structured-data-card')).toBeInTheDocument()
      expect(screen.getByTestId('structured-data-card')).toHaveAttribute('data-type', 'unknown')
    })

    it('passes full XML content to StructuredDataCard', () => {
      const xml = `<data><field>value</field></data>`

      render(<XmlCard content={xml} type="unknown" />)

      const card = screen.getByTestId('structured-data-card')
      expect(card.textContent).toContain('<data><field>value</field></data>')
    })

    it('does not render CodeBlock for unknown type', () => {
      const xml = `<some_xml><nested>content</nested></some_xml>`

      const { container } = render(<XmlCard content={xml} type="unknown" />)

      expect(container.querySelector('[data-testid="structured-data-card"]')).toBeInTheDocument()
      expect(container.querySelector('code')).not.toBeInTheDocument()
    })
  })

  describe('existing card types still work', () => {
    it('renders nothing for hidden type', () => {
      const { container } = render(<XmlCard content="<system-reminder>hidden</system-reminder>" type="hidden" />)
      expect(container.innerHTML).toBe('')
    })

    it('renders local_command as terminal output', () => {
      const xml = `<local-command-stdout>hello world</local-command-stdout>`
      render(<XmlCard content={xml} type="local_command" />)
      expect(screen.getByText('hello world')).toBeInTheDocument()
    })

    it('renders task_notification as agent status card', () => {
      const xml = `<task-notification>
        <status>completed</status>
        <summary>Task done</summary>
        <result>Success</result>
      </task-notification>`
      render(<XmlCard content={xml} type="task_notification" />)
      expect(screen.getByText('Task done')).toBeInTheDocument()
    })

    it('renders tool_error as red error card', () => {
      const xml = `<tool_use_error>File not found</tool_use_error>`
      render(<XmlCard content={xml} type="tool_error" />)
      expect(screen.getByText('Tool Error')).toBeInTheDocument()
      expect(screen.getAllByText('File not found').length).toBeGreaterThanOrEqual(1)
    })

    it('renders command as indigo card', () => {
      const xml = `<command-name>git status</command-name>
        <command-message>Check status</command-message>
        <command-args>--short</command-args>`
      render(<XmlCard content={xml} type="command" />)
      expect(screen.getByText('git status')).toBeInTheDocument()
    })

    it('renders observation with parsed facts', () => {
      const xml = `<observation>
        <type>Architecture</type>
        <title>Project structure</title>
        <facts><fact>Uses React</fact><fact>Uses TypeScript</fact></facts>
      </observation>`
      render(<XmlCard content={xml} type="observation" />)
      expect(screen.getByText(/Architecture · Project structure/)).toBeInTheDocument()
    })

    it('renders observed_from_primary_session with action summary', () => {
      const xml = `<observed_from_primary_session>
        <what_happened>Read file</what_happened>
        <parameters>/src/index.ts</parameters>
      </observed_from_primary_session>`
      render(<XmlCard content={xml} type="observed_from_primary_session" />)
      expect(screen.getByText(/Read file/)).toBeInTheDocument()
    })
  })

  describe('detectXmlType', () => {
    it('detects tool_call type', () => {
      expect(detectXmlType('<tool_call><name>Read</name></tool_call>')).toBe('tool_call')
    })

    it('detects unknown type for generic XML', () => {
      const xml = '<custom_structure>' + 'a'.repeat(100) + '</custom_structure>'
      expect(detectXmlType(xml)).toBe('unknown')
    })

    it('returns null for non-XML content', () => {
      expect(detectXmlType('just plain text')).toBeNull()
    })
  })

  describe('extractXmlBlocks', () => {
    it('extracts tool_call blocks with correct type', () => {
      const content = 'Some text <tool_call><name>Read</name></tool_call> more text'
      const blocks = extractXmlBlocks(content)
      const toolBlock = blocks.find(b => b.type === 'tool_call')
      expect(toolBlock).toBeDefined()
      expect(toolBlock!.xml).toContain('<tool_call>')
    })
  })
})
