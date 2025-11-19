import { useWorkflowStore } from '../store/useWorkflowStore';
import { X } from 'lucide-react';

const PropertiesPanel = () => {
  const { 
    nodes, edges, 
    selectedNodeId, selectedEdgeId, 
    updateNodeData, updateEdgeLabel, 
    selectNode, selectEdge 
  } = useWorkflowStore();

  const selectedNode = nodes.find((n) => n.id === selectedNodeId);
  const selectedEdge = edges.find((e) => e.id === selectedEdgeId);

  const clearSelection = () => {
    selectNode(null);
    selectEdge(null);
  };

  if (selectedEdge) {
    return (
      <aside className="w-80 bg-white border-l border-gray-200 flex flex-col h-full shadow-lg z-10">
         <div className="p-4 border-b border-gray-100 flex justify-between items-center bg-gray-50">
            <h2 className="font-bold text-gray-700">Edge Properties</h2>
            <button onClick={clearSelection} className="text-gray-400 hover:text-gray-600">
                <X size={16}/>
            </button>
        </div>
        <div className="p-4">
             <div>
                <label className="block text-xs font-bold text-gray-500 mb-1">Condition / Label</label>
                <input
                    className="w-full border border-gray-300 rounded px-3 py-2 text-sm font-mono focus:ring-2 focus:ring-blue-500 outline-none"
                    value={(selectedEdge.label as string) || ''}
                    onChange={(e) => updateEdgeLabel(selectedEdge.id, e.target.value)}
                    placeholder="${x} == 1"
                />
                <p className="text-xs text-gray-400 mt-1">Use this for 'If' branches.</p>
            </div>
        </div>
      </aside>
    );
  }

  if (!selectedNode) {
    return (
      <aside className="w-80 bg-gray-50 border-l border-gray-200 p-4 flex flex-col items-center justify-center text-gray-400">
        <div className="text-sm">Select a node or edge to edit</div>
      </aside>
    );
  }

  const handleChange = (field: string, value: any) => {
    updateNodeData(selectedNode.id, { [field]: value });
  };

  return (
    <aside className="w-80 bg-white border-l border-gray-200 flex flex-col h-full shadow-lg z-10">
      <div className="p-4 border-b border-gray-100 flex justify-between items-center bg-gray-50">
        <h2 className="font-bold text-gray-700">Properties</h2>
        <button onClick={clearSelection} className="text-gray-400 hover:text-gray-600">
            <X size={16}/>
        </button>
      </div>
      
      <div className="p-4 overflow-y-auto flex-1 space-y-4">
        
        <div>
            <label className="block text-xs font-bold text-gray-500 mb-1">Label</label>
            <input
            className="w-full border border-gray-300 rounded px-3 py-2 text-sm focus:ring-2 focus:ring-blue-500 outline-none"
            value={selectedNode.data.label || ''}
            onChange={(e) => handleChange('label', e.target.value)}
            />
        </div>

        <div className="p-3 bg-gray-100 rounded text-xs text-gray-500 font-mono break-all">
            ID: {selectedNode.id}
        </div>

        <hr className="border-gray-100" />

        {/* Node Specific Fields */}
        {selectedNode.data.kind === 'Function' && (
            <>
            <div>
                <label className="block text-xs font-bold text-gray-500 mb-1">Function Name</label>
                <input
                className="w-full border border-gray-300 rounded px-3 py-2 text-sm focus:ring-2 focus:ring-blue-500 outline-none"
                value={selectedNode.data.functionName || ''}
                onChange={(e) => handleChange('functionName', e.target.value)}
                placeholder="e.g. http_request"
                />
            </div>
            <div>
                <label className="block text-xs font-bold text-gray-500 mb-1">Parameters (JSON)</label>
                <textarea
                className="w-full border border-gray-300 rounded px-3 py-2 text-sm font-mono h-32 focus:ring-2 focus:ring-blue-500 outline-none resize-none"
                value={selectedNode.data.params || ''}
                onChange={(e) => handleChange('params', e.target.value)}
                placeholder='{ "url": "..." }'
                />
            </div>
            <div>
                <label className="block text-xs font-bold text-gray-500 mb-1">Output Variable</label>
                <input
                className="w-full border border-gray-300 rounded px-3 py-2 text-sm focus:ring-2 focus:ring-blue-500 outline-none"
                value={selectedNode.data.outputVar || ''}
                onChange={(e) => handleChange('outputVar', e.target.value)}
                />
            </div>
            </>
        )}

        {selectedNode.data.kind === 'Assign' && (
            <div>
                <label className="block text-xs font-bold text-gray-500 mb-1">Expression</label>
                <input
                className="w-full border border-gray-300 rounded px-3 py-2 text-sm font-mono focus:ring-2 focus:ring-blue-500 outline-none"
                value={selectedNode.data.expression || ''}
                onChange={(e) => handleChange('expression', e.target.value)}
                placeholder="x = y + 1"
                />
            </div>
        )}

        {(selectedNode.data.kind === 'If' || selectedNode.data.kind === 'Loop') && (
             <div>
                <label className="block text-xs font-bold text-gray-500 mb-1">Condition</label>
                <input
                className="w-full border border-gray-300 rounded px-3 py-2 text-sm font-mono focus:ring-2 focus:ring-blue-500 outline-none"
                value={selectedNode.data.condition || ''}
                onChange={(e) => handleChange('condition', e.target.value)}
                placeholder="${var} == true"
                />
            </div>
        )}
        
        {selectedNode.data.kind === 'Iteration' && (
            <>
             <div>
                <label className="block text-xs font-bold text-gray-500 mb-1">Collection</label>
                <input
                className="w-full border border-gray-300 rounded px-3 py-2 text-sm font-mono focus:ring-2 focus:ring-blue-500 outline-none"
                value={selectedNode.data.collection || ''}
                onChange={(e) => handleChange('collection', e.target.value)}
                />
            </div>
            <div>
                <label className="block text-xs font-bold text-gray-500 mb-1">Item Variable</label>
                <input
                className="w-full border border-gray-300 rounded px-3 py-2 text-sm font-mono focus:ring-2 focus:ring-blue-500 outline-none"
                value={selectedNode.data.itemVar || ''}
                onChange={(e) => handleChange('itemVar', e.target.value)}
                />
            </div>
            </>
        )}
      </div>
      
    </aside>
  );
};

export default PropertiesPanel;
