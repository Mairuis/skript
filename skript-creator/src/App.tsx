import React, { useRef } from 'react';
import Sidebar from './components/Sidebar';
import Canvas from './components/Canvas';
import PropertiesPanel from './components/PropertiesPanel';
import { useWorkflowStore } from './store/useWorkflowStore';
import { Download, Upload, Play } from 'lucide-react';
import { exportToYaml, importFromYaml } from './utils/yamlConverter';

function App() {
  const { nodes, edges, workflowName, workflowId, setNodes, setEdges, setWorkflowMeta } = useWorkflowStore();
  const fileInputRef = useRef<HTMLInputElement>(null);

  const handleExport = () => {
    const yamlString = exportToYaml(nodes, edges, { id: workflowId, name: workflowName });
    const blob = new Blob([yamlString], { type: 'text/yaml' });
    const url = URL.createObjectURL(blob);
    const link = document.createElement('a');
    link.href = url;
    link.download = `${workflowId}.yaml`;
    document.body.appendChild(link);
    link.click();
    document.body.removeChild(link);
  };

  const handleImportClick = () => {
    fileInputRef.current?.click();
  };

  const handleFileChange = (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (!file) return;

    const reader = new FileReader();
    reader.onload = (e) => {
      const content = e.target?.result as string;
      try {
        const { nodes: newNodes, edges: newEdges, meta } = importFromYaml(content);
        setNodes(newNodes);
        setEdges(newEdges);
        setWorkflowMeta(meta.id, meta.name);
      } catch (error) {
        console.error("Failed to parse YAML", error);
        alert("Failed to parse YAML file.");
      }
    };
    reader.readAsText(file);
  };

  return (
    <div className="flex flex-col h-screen text-slate-900 font-sans">
      {/* Header */}
      <header className="h-14 bg-white border-b border-gray-200 flex items-center px-4 justify-between z-20 shadow-sm">
        <div className="flex items-center space-x-3">
            <div className="bg-indigo-600 w-8 h-8 rounded-lg flex items-center justify-center text-white font-bold shadow-indigo-200 shadow-md">
                <Play size={16} fill="white" />
            </div>
            <div>
                <h1 className="font-bold text-gray-800 leading-tight">Skript Creator</h1>
                <div className="text-xs text-gray-400 font-medium">{workflowName}</div>
            </div>
        </div>
        <div className="flex space-x-2">
            <input 
                type="file" 
                ref={fileInputRef} 
                onChange={handleFileChange} 
                accept=".yaml,.yml" 
                className="hidden" 
            />
            <button 
                onClick={handleImportClick}
                className="flex items-center px-3 py-2 text-xs font-medium bg-white border border-gray-300 hover:bg-gray-50 rounded-md text-gray-700 transition-colors"
            >
                <Upload size={14} className="mr-2"/> Import
            </button>
            <button 
                onClick={handleExport}
                className="flex items-center px-3 py-2 text-xs font-medium bg-indigo-600 hover:bg-indigo-700 rounded-md text-white shadow-sm transition-colors"
            >
                <Download size={14} className="mr-2"/> Export YAML
            </button>
        </div>
      </header>

      {/* Main Content */}
      <div className="flex-1 flex overflow-hidden">
        <Sidebar />
        <Canvas />
        <PropertiesPanel />
      </div>
    </div>
  );
}

export default App;