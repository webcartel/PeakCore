import { afterEach, describe, expect, it } from 'vitest'
import { formatBytes, pluralForm, setLocale, t, tn } from './i18n'

afterEach(() => setLocale('ru'))

describe('russian plurals', () => {
  it('picks the form by the last digits', () => {
    expect(pluralForm(1)).toBe('one')
    expect(pluralForm(21)).toBe('one')
    expect(pluralForm(2)).toBe('few')
    expect(pluralForm(4)).toBe('few')
    expect(pluralForm(5)).toBe('many')
    expect(pluralForm(11)).toBe('many')
    expect(pluralForm(12)).toBe('many')
    expect(pluralForm(0)).toBe('many')
  })

  it('renders the right word for process counts', () => {
    setLocale('ru')
    expect(tn('overview.processCount', 1)).toBe('1 процесс')
    expect(tn('overview.processCount', 2)).toBe('2 процесса')
    expect(tn('overview.processCount', 5)).toBe('5 процессов')
    expect(tn('overview.processCount', 11)).toBe('11 процессов')
    expect(tn('overview.processCount', 21)).toBe('21 процесс')
  })

  it('falls back to one/many in english', () => {
    setLocale('en')
    expect(tn('overview.processCount', 1)).toBe('1 process')
    expect(tn('overview.processCount', 2)).toBe('2 processes')
    expect(tn('overview.processCount', 21)).toBe('21 processes')
  })
})

describe('localised formatting', () => {
  it('uses the active unit names', () => {
    setLocale('ru')
    expect(formatBytes(1024 ** 3 * 1.5)).toBe('1.5 ГБ')
    expect(formatBytes(1024 ** 2 * 512)).toBe('512 МБ')
    setLocale('en')
    expect(formatBytes(1024 ** 3 * 1.5)).toBe('1.5 GB')
    expect(formatBytes(1024 ** 2 * 512)).toBe('512 MB')
  })

  it('interpolates parameters', () => {
    setLocale('en')
    expect(t('cpu.coreTitle', { index: 3, value: 42 })).toBe('Core 3: 42%')
  })
})
