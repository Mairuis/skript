import React, { useCallback, useRef } from 'react';
import ReactFlow, {
  Background,
  Controls,
  ReactFlowProvider,
  useReactFlow,
  MiniMap
} from 'reactflow';
import 'reactflow/dist/style.css';

import { useWorkflowStore } from '../store/useWorkflowStore';
import GenericNode from './nodes/GenericNode';
import type { NodeType } from '../types';

const nodeTypes = {
  Start: GenericNode,
  End: GenericNode,
  Function: GenericNode,
  Assign: GenericNode,
  If: GenericNode,
  Parallel: GenericNode,
  Iteration: GenericNode,
  Loop: GenericNode,
};

const Canvas = () => {
  const reactFlowWrapper = useRef<HTMLDivElement>(null);
  const {
    nodes,
    edges,
    onNodesChange,
    onEdgesChange,
    onConnect,
    addNode,
    selectNode,
    selectEdge
  } = useWorkflowStore();
  
  const { project } = useReactFlow();

  const onDragOver = useCallback((event: React.DragEvent) => {
    event.preventDefault();
    event.dataTransfer.dropEffect = 'move';
  }, []);

  const onDrop = useCallback(
    (event: React.DragEvent) => {
      event.preventDefault();

      const type = event.dataTransfer.getData('application/reactflow') as NodeType;

      if (typeof type === 'undefined' || !type) {
        return;
      }

      const position = project({
        x: event.clientX - (reactFlowWrapper.current?.getBoundingClientRect().left ?? 0),
        y: event.clientY - (reactFlowWrapper.current?.getBoundingClientRect().top ?? 0),
      });
      
      const newNode = {
        id: `${type.toLowerCase()}_${Math.random().toString(36).substr(2, 9)}`,
        type: type, // React Flow type
        position,
        data: { kind: type, label: type },
      };

      addNode(newNode);
    },
    [project, addNode]
  );

  const onNodeClick = useCallback((_: React.MouseEvent, node: any) => {
    selectNode(node.id);
  }, [selectNode]);

  const onEdgeClick = useCallback((_: React.MouseEvent, edge: any) => {
    selectEdge(edge.id);
  }, [selectEdge]);
  
  const onPaneClick = useCallback(() => {
    selectNode(null);
    selectEdge(null);
  }, [selectNode, selectEdge]);

  return (
    <div className="flex-1 h-full bg-slate-50" ref={reactFlowWrapper}>
      <ReactFlow
        nodes={nodes}
        edges={edges}
        onNodesChange={onNodesChange}
        onEdgesChange={onEdgesChange}
        onConnect={onConnect}
        nodeTypes={nodeTypes}
        onDragOver={onDragOver}
        onDrop={onDrop}
        onNodeClick={onNodeClick}
        onEdgeClick={onEdgeClick}
        onPaneClick={onPaneClick}
        fitView
      >
        <Background color="#cbd5e1" gap={20} size={1} />
        <Controls className="bg-white border border-gray-200 shadow-sm" />
        <MiniMap className="border border-gray-200 shadow-sm" />
      </ReactFlow>
    </div>
  );
};

export default () => (
    <ReactFlowProvider>
        <Canvas />
    </ReactFlowProvider>
);
