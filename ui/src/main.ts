import { createApp } from 'vue'
import { createPinia } from 'pinia'
import { createRouter, createWebHashHistory } from 'vue-router'
import App from './App.vue'
import OverviewPage from './pages/OverviewPage.vue'
import ProcessesPage from './pages/ProcessesPage.vue'
import DiskPage from './pages/DiskPage.vue'
import NetworkPage from './pages/NetworkPage.vue'
import CpuPage from './pages/CpuPage.vue'
import MemoryPage from './pages/MemoryPage.vue'
import GpuPage from './pages/GpuPage.vue'
import SettingsPage from './pages/SettingsPage.vue'
import AlertsPage from './pages/AlertsPage.vue'
import PlaceholderPage from './pages/PlaceholderPage.vue'
import './style.css'

const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: '/', component: OverviewPage, meta: { titleKey: 'nav.overview' } },
    { path: '/processes', component: ProcessesPage, meta: { titleKey: 'nav.processes' } },
    { path: '/disk', component: DiskPage, meta: { titleKey: 'nav.disk' } },
    { path: '/network', component: NetworkPage, meta: { titleKey: 'nav.network' } },
    { path: '/cpu', component: CpuPage, meta: { titleKey: 'nav.cpu' } },
    { path: '/memory', component: MemoryPage, meta: { titleKey: 'nav.memory' } },
    { path: '/gpu', component: GpuPage, meta: { titleKey: 'nav.gpu' } },
    { path: '/alerts', component: AlertsPage, meta: { titleKey: 'nav.alerts' } },
    { path: '/settings', component: SettingsPage, meta: { titleKey: 'nav.settings' } },
    { path: '/:section(.*)', component: PlaceholderPage },
  ],
})

createApp(App).use(createPinia()).use(router).mount('#app')
