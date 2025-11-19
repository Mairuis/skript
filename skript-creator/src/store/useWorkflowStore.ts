import { create } from 'zustand';
import {
  type Connection,
  type Edge,
  type EdgeChange,
  type NodeChange,
  addEdge,
  type OnNodesChange,
  type OnEdgesChange,
  type OnConnect,
  applyNodeChanges,
  applyEdgeChanges,
} from 'reactflow';
import type { SkriptNode } from '../types';

type WorkflowState = {
  nodes: SkriptNode[];
  edges: Edge[];
  workflowId: string;
  workflowName: string;
  selectedNodeId: string | null;
  selectedEdgeId: string | null;

  onNodesChange: OnNodesChange;
  onEdgesChange: OnEdgesChange;
  onConnect: OnConnect;
  
  setNodes: (nodes: SkriptNode[]) => void;
  setEdges: (edges: Edge[]) => void;
  addNode: (node: SkriptNode) => void;
  updateNodeData: (id: string, data: Partial<SkriptNode['data']>) => void;
  updateEdgeLabel: (id: string, label: string) => void;
  
  selectNode: (id: string | null) => void;
  selectEdge: (id: string | null) => void;
  setWorkflowMeta: (id: string, name: string) => void;
};

export const useWorkflowStore = create<WorkflowState>((set, get) => ({
  nodes: [],
  edges: [],
  workflowId: 'new-workflow',
  workflowName: 'My Workflow',
  selectedNodeId: null,
  selectedEdgeId: null,

  onNodesChange: (changes: NodeChange[]) => {
    set({
      nodes: applyNodeChanges(changes, get().nodes) as SkriptNode[],
    });
  },

  onEdgesChange: (changes: EdgeChange[]) => {
    set({
      edges: applyEdgeChanges(changes, get().edges),
    });
  },

  onConnect: (connection: Connection) => {
    set({
      edges: addEdge({ ...connection, type: 'default' }, get().edges),
    });
  },

  setNodes: (nodes) => set({ nodes }),
  setEdges: (edges) => set({ edges }),
  
  addNode: (node) => set({ nodes: [...get().nodes, node] }),
  
  updateNodeData: (id, data) => {
    set({
      nodes: get().nodes.map((node) =>
        node.id === id
          ? { ...node, data: { ...node.data, ...data } }
          : node
      ),
    });
  },

  updateEdgeLabel: (id, label) => {
    set({
      edges: get().edges.map((edge) =>
        edge.id === id
          ? { ...edge, label: label || undefined, data: { ...edge.data, condition: label } }
          : edge
      ),
    });
  },

  selectNode: (id) => set({ selectedNodeId: id, selectedEdgeId: null }),
  selectEdge: (id) => set({ selectedEdgeId: id, selectedNodeId: null }),
  setWorkflowMeta: (id, name) => set({ workflowId: id, workflowName: name }),
}));
