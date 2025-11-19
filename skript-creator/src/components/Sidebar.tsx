import React from 'react';
import type { NodeType } from '../types';

const nodeTypes: NodeType[] = [
  'Function', 'Assign', 'If', 'Parallel', 'Iteration', 'Loop', 'End'
];

const Sidebar = () => {
  const onDragStart = (event: React.DragEvent, nodeType: NodeType) => {
    event.dataTransfer.setData('application/reactflow', nodeType);
    event.dataTransfer.effectAllowed = 'move';
  };

  return (
    <aside className="w-64 bg-gray-50 border-r border-gray-200 p-4 flex flex-col z-10">
      <h2 className="text-xs font-bold uppercase tracking-wider mb-4 text-gray-500">Nodes Library</h2>
      <div className="space-y-2">
        <div
            className="p-3 bg-green-50 border border-green-200 rounded cursor-move text-sm font-medium text-green-800 hover:bg-green-100 transition-colors flex items-center"
            onDragStart={(event) => onDragStart(event, 'Start')}
            draggable
        >
            Start Node
        </div>
        {nodeTypes.map((type) => (
          <div
            key={type}
            className="p-3 bg-white border border-gray-200 rounded cursor-move text-sm font-medium text-gray-700 hover:border-blue-300 hover:shadow-sm transition-all"
            onDragStart={(event) => onDragStart(event, type)}
            draggable
          >
            {type}
          </div>
        ))}
      </div>
    </aside>
  );
};

export default Sidebar;
