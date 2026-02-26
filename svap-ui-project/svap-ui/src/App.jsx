import React, { useState } from 'react';
import { PipelineProvider } from './data/usePipelineData';
import Sidebar from './components/Sidebar';
import Dashboard from './views/Dashboard';
import CaseSourcing from './views/CaseSourcing';
import PolicyExplorer from './views/PolicyExplorer';
import TaxonomyView from './views/TaxonomyView';
import ConvergenceMatrix from './views/ConvergenceMatrix';
import PredictionView from './views/PredictionView';
import DetectionView from './views/DetectionView';

const VIEWS = {
  dashboard: Dashboard,
  cases: CaseSourcing,
  policies: PolicyExplorer,
  taxonomy: TaxonomyView,
  matrix: ConvergenceMatrix,
  predictions: PredictionView,
  detection: DetectionView,
};

export default function App() {
  const [activeView, setActiveView] = useState('dashboard');
  const ViewComponent = VIEWS[activeView] || Dashboard;

  return (
    <PipelineProvider>
      <div className="app-layout">
        <Sidebar activeView={activeView} onNavigate={setActiveView} />
        <main className="main-content" key={activeView}>
          <ViewComponent onNavigate={setActiveView} />
        </main>
      </div>
    </PipelineProvider>
  );
}
