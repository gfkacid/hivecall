import { Link, Outlet } from 'react-router-dom'

export function Layout() {
  return (
    <div className="layout">
      <header className="header">
        <strong>HiveCall</strong>
        <nav>
          <Link to="/">Vaults</Link>
          <Link to="/native">Native vault</Link>
          <Link to="/create">Create vault</Link>
        </nav>
      </header>
      <main className="main">
        <Outlet />
      </main>
    </div>
  )
}
