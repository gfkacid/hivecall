import { BrowserRouter, Navigate, Route, Routes } from 'react-router-dom'
import { Layout } from './components/Layout'
import { CreateVaultPage } from './pages/CreateVaultPage'
import { HomePage } from './pages/HomePage'
import { NativeVaultPage } from './pages/NativeVaultPage'
import { VaultDetailPage } from './pages/VaultDetailPage'
import './App.css'

export default function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route path="/" element={<Layout />}>
          <Route index element={<HomePage />} />
          <Route path="native" element={<NativeVaultPage />} />
          <Route path="create" element={<CreateVaultPage />} />
          <Route path="vault/:vaultId" element={<VaultDetailPage />} />
          <Route path="*" element={<Navigate to="/" replace />} />
        </Route>
      </Routes>
    </BrowserRouter>
  )
}
