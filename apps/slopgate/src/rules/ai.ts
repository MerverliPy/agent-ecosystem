// AI-phrasing rules: unmistakable AI-assistant boilerplate in prose.
import type { Finding, TextRule } from '../types.ts';

const category = 'ai-phrasing' as const;

interface AiPattern {
  id: string;
  name: string;
  severity: Finding['severity'];
  description: string;
  patterns: RegExp[];
  message: string;
}

const PATTERNS: AiPattern[] = [
  {
    id: 'AI-001',
    name: '"As an AI language model"',
    severity: 'high',
    description: 'The canonical ChatGPT refusal/disclaimer phrasing.',
    patterns: [/\bas an ai(?: language model)?\b/i, /\bas an artificial intelligence\b/i],
    message: 'Found "As an AI language model" phrasing.',
  },
  {
    id: 'AI-002',
    name: '"As a language model"',
    severity: 'high',
    description: 'The same disclaimer in its shorter form.',
    patterns: [/\bas (?:a|an) (?:large )?language model\b/i],
    message: 'Found "As a language model" phrasing.',
  },
  {
    id: 'AI-003',
    name: 'Refusal boilerplate',
    severity: 'high',
    description: 'Assistant refusal templates: "I cannot assist with…", "I am unable to…".',
    patterns: [
      /\bi(?:'m| am) sorry,? but (?:i|we) (?:can'?t|cannot|won'?t)\b/i,
      /\bi (?:can'?t|cannot|am unable to|am not able to|don'?t have the ability to) (?:assist|help|provide|fulfill|comply)\b/i,
      /\bi (?:can'?t|cannot) (?:assist|help) with (?:this|that|such)\b/i,
      /\bi'?m (?:sorry|afraid) (?:that )?i (?:can'?t|cannot)\b/i,
    ],
    message: 'Found assistant refusal boilerplate.',
  },
  {
    id: 'AI-004',
    name: 'No-opinion disclaimer',
    severity: 'medium',
    description: '"I don\'t have personal opinions/beliefs" — assistant boilerplate.',
    patterns: [
      /\bi (?:don'?t|do not) have (?:any )?(?:personal )?(?:opinions?|beliefs?|feelings?|experiences?|preferences?)\b/i,
      /\bi (?:am|'m) (?:an ai|a language model).{0,40}no personal/i,
    ],
    message: 'Found no-opinion AI disclaimer.',
  },
  {
    id: 'AI-005',
    name: 'Training-data / cutoff phrasing',
    severity: 'medium',
    description: '"As of my last knowledge update", "my training data" — model self-references.',
    patterns: [
      /\bas of my (?:last )?knowledge (?:update|cutoff)\b/i,
      /\bmy training (?:data|corpus|cutoff)\b/i,
      /\bmy knowledge cutoff\b/i,
      /\bmy (?:training )?data only goes up to\b/i,
    ],
    message: 'Found model training-data self-reference.',
  },
  {
    id: 'AI-006',
    name: '"Let me know if you have other questions"',
    severity: 'low',
    description: 'The generic assistant sign-off.',
    patterns: [
      /\blet me know if you have (?:any )?(?:other |further )?questions?\b/i,
      /\bfeel free to (?:ask|reach out|contact me)\b/i,
      /\bif you have (?:any )?(?:more |further |other )?questions?\b/i,
    ],
    message: 'Found generic assistant sign-off.',
  },
  {
    id: 'AI-007',
    name: '"Certainly! Here\'s…"',
    severity: 'low',
    description: 'The eager assistant preamble before a generated answer.',
    patterns: [
      /\b(?:certainly|absolutely|sure|of course|great)!\s*(?:here|here'?s|i can|i'?ll|let me)\b/i,
      /\bhere'?s (?:a |an |the )?step-by-step\b/i,
      /\bsure, here'?s\b/i,
    ],
    message: 'Found assistant preamble ("Certainly! Here\'s…").',
  },
  {
    id: 'AI-008',
    name: '"Is there anything else I can help with?"',
    severity: 'low',
    description: 'The follow-up assistant close.',
    patterns: [
      /\bis there anything else i can (?:help|assist) (?:you )?(?:with)?\??\b/i,
      /\banything else (?:i|we) can help\b/i,
    ],
    message: 'Found follow-up assistant close.',
  },
  {
    id: 'AI-009',
    name: '"It\'s important to note…"',
    severity: 'low',
    description: 'Hedging AI-essay filler.',
    patterns: [
      /\bit'?s (?:important|worth|essential|crucial) (?:to note|noting|to remember|to keep in mind)\b/i,
      /\bit is (?:important|worth) (?:to note|noting)\b/i,
    ],
    message: 'Found hedging filler ("it\'s important to note").',
  },
  {
    id: 'AI-010',
    name: '"I\'d be happy to…"',
    severity: 'low',
    description: 'Eager-assistant boilerplate.',
    patterns: [
      /\bi'?d be happy to\b/i,
      /\bi'?m happy to help\b/i,
      /\bi'?d be glad to\b/i,
      /\b(?:i'?m|i am) (?:here|happy) to (?:assist|help)\b/i,
    ],
    message: 'Found eager-assistant boilerplate.',
  },
  {
    id: 'AI-011',
    name: '"Great question!"',
    severity: 'low',
    description: 'Assistant flattery opening.',
    patterns: [/\b(?:great|good|excellent) (?:question|point)!/i],
    message: 'Found assistant flattery ("great question!").',
  },
  {
    id: 'AI-012',
    name: 'Essay wrap-up filler',
    severity: 'low',
    description: '"In conclusion", "to summarize", "in summary" — AI-essay transitions.',
    patterns: [/\bin conclusion\b/i, /\bto summarize\b/i, /\bin summary\b/i, /\ball things considered\b/i],
    message: 'Found essay wrap-up filler.',
  },
  {
    id: 'AI-013',
    name: '"Here is a comprehensive…"',
    severity: 'low',
    description: 'AI essay bloat ("comprehensive guide", "detailed breakdown").',
    patterns: [
      /\bhere (?:is|are|'?s) a (?:comprehensive|detailed|thorough|in-?depth)\b/i,
      /\ba comprehensive (?:guide|overview|analysis|breakdown)\b/i,
      /\b(?:delve|diving) (?:into|deep)\b/i,
    ],
    message: 'Found AI essay bloat ("here is a comprehensive…").',
  },
  {
    id: 'AI-014',
    name: '"Please note that"',
    severity: 'low',
    description: 'Formal-email/AI hedging.',
    patterns: [/\bplease note that\b/i, /\bkindly note\b/i],
    message: 'Found hedging ("please note that").',
  },
];

export function aiPhrasingRules(): TextRule[] {
  return PATTERNS.map((p) => ({
    id: p.id,
    name: p.name,
    category,
    severity: p.severity,
    description: p.description,
    check(text: string, source: string): Finding[] {
      const out: Finding[] = [];
      for (const re of p.patterns) {
        const m = text.match(re);
        if (m) {
          const start = Math.max(0, (m.index ?? 0) - 40);
          const end = Math.min(text.length, (m.index ?? 0) + m[0].length + 40);
          out.push({
            ruleId: p.id,
            severity: p.severity,
            category,
            message: p.message,
            evidence: text.slice(start, end).trim(),
          });
          break;
        }
      }
      return out;
    },
  }));
}
