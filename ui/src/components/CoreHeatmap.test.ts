import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import CoreHeatmap from './CoreHeatmap.vue'

describe('CoreHeatmap', () => {
  it('renders one tile per core with the load level', () => {
    const wrapper = mount(CoreHeatmap, { props: { cores: [10, 70, 95] } })
    const tiles = wrapper.findAll('.core-tile')
    expect(tiles).toHaveLength(3)
    expect(tiles[0].classes()).toContain('ok')
    expect(tiles[1].classes()).toContain('warn')
    expect(tiles[2].classes()).toContain('crit')
    expect(tiles[2].text()).toContain('95')
  })

  it('explains the empty state instead of rendering tiles', () => {
    const wrapper = mount(CoreHeatmap, { props: { cores: [] } })
    expect(wrapper.findAll('.core-tile')).toHaveLength(0)
    expect(wrapper.find('.core-heatmap-empty').exists()).toBe(true)
  })
})
