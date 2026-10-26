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
import PlaceholderPage from './pages/PlaceholderPage.vue'
import './style.css'

const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: '/', component: OverviewPage, meta: { title: 'Обзор' } },
    { path: '/processes', component: ProcessesPage, meta: { title: 'Процессы' } },
    { path: '/disk', component: DiskPage, meta: { title: 'Диск' } },
    { path: '/network', component: NetworkPage, meta: { title: 'Сеть' } },
    { path: '/cpu', component: CpuPage, meta: { title: 'CPU' } },
    { path: '/memory', component: MemoryPage, meta: { title: 'Память' } },
    { path: '/:section(.*)', component: PlaceholderPage },
  ],
})

createApp(App).use(createPinia()).use(router).mount('#app')
