// The MCP tool surface mirrors the /api closed loop 1:1.

import { describe, expect, test } from 'bun:test'

import { buildTools } from '../mcp.js'

describe('mcp tools', () => {
  const tools = buildTools()

  test('exactly the four closed-loop tools exist', () => {
    expect(tools.map((t) => t.name).sort()).toEqual([
      'sentori_issue_bundle',
      'sentori_issue_list',
      'sentori_issue_note',
      'sentori_issue_resolve',
    ])
  })

  test('every tool has a description and an object schema', () => {
    for (const t of tools) {
      expect(t.description.length).toBeGreaterThan(20)
      expect((t.inputSchema as { type?: string }).type).toBe('object')
    }
  })

  test('bundle/note/resolve require issueId', () => {
    for (const name of ['sentori_issue_bundle', 'sentori_issue_note', 'sentori_issue_resolve']) {
      const t = tools.find((x) => x.name === name)!
      expect((t.inputSchema as { required?: string[] }).required).toContain('issueId')
    }
  })
})
