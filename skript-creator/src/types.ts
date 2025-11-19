import type { Node } from 'reactflow';

export type NodeType =
  | 'Start'
  | 'End'
  | 'Function'
  | 'Assign'
  | 'If'
  | 'Parallel'
  | 'Iteration'
  | 'Loop';

export interface SkriptNodeData {
  label?: string;
  kind: NodeType;
  
  // Function
  functionName?: string;
  params?: string; // Storing as JSON string or parsing it? Let's store as string for simple text editing first, or object if we build a form.
  // YAML usually has params as object. Let's store as Object but the UI might need to handle it.
  paramsObj?: Record<string, any>; 
  
  outputVar?: string;
  
  // Assign
  expression?: string;
  // assignments?: Record<string, any>[]; // Complicated UI, maybe just use expression for now as it's primary in example
  
  // If / Loop
  condition?: string;
  
  // Iteration
  collection?: string;
  itemVar?: string;

  // Parallel
  // Branches are handled via structure, but maybe we need metadata here?
}

export type SkriptNode = Node<SkriptNodeData>;
