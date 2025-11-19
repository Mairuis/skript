import yaml from 'js-yaml';
import type { Edge } from 'reactflow';
import type { SkriptNode } from '../types';

// --- EXPORT ---

export const exportToYaml = (nodes: SkriptNode[], edges: Edge[], meta: { id: string, name: string }) => {
  // 1. Build DSL Nodes
  const dslNodes = nodes.map(node => {
    const { kind, label, ...params } = node.data;
    
    const dslNode: any = {
      id: node.id, // Use ID as label if label matches? No, use Node ID.
      type: kind,
      ...params
    };

    // Try to parse params back to object if it's a string
    if (dslNode.params && typeof dslNode.params === 'string') {
        try {
            dslNode.params = JSON.parse(dslNode.params);
        } catch (e) {
            // ignore
        }
    }

    // Clean up UI specific fields
    delete dslNode.label;
    delete dslNode.kind;

    return dslNode;
  });

  // 2. Build DSL Edges
  const dslEdges = edges.map(edge => {
      const e: any = {
        source: edge.source,
        target: edge.target,
      };
      if (edge.data?.condition || edge.label) {
          e.condition = edge.data?.condition || edge.label;
      }
      return e;
  });

  return yaml.dump({
    workflow: {
      id: meta.id,
      name: meta.name,
      // variables: {} // TODO: Add variables editor
    },
    nodes: dslNodes,
    edges: dslEdges
  });
};

// --- IMPORT ---

export const importFromYaml = (yamlString: string) => {
  const data: any = yaml.load(yamlString);
  
  const nodes: SkriptNode[] = [];
  const edges: Edge[] = [];
  
  let yPos = 50;
  let xPos = 250;
  
  // Helper to flatten nodes if nested (recursion)
  const processNode = (node: any, xOffset = 0) => {
     const { id, type, kind, next, branches, ...rest } = node;
     const nodeType = type || kind || 'Function'; // Fallback

     // Check if node already exists (to avoid duplicates if ref'd multiple times)
     if (nodes.find(n => n.id === id)) return;

     const rfNode: SkriptNode = {
        id: id,
        type: nodeType, 
        position: { x: xPos + xOffset, y: yPos },
        data: {
            kind: nodeType,
            label: id,
            ...rest
        }
     };
     
     // Handle Params
     if (rest.params && typeof rest.params === 'object') {
         rfNode.data.params = JSON.stringify(rest.params, null, 2);
     }

     nodes.push(rfNode);
     yPos += 120;

     // Handle 'next' field -> convert to Edge
     if (next) {
        edges.push({
            id: `e_${id}_${next}`,
            source: id,
            target: next,
            type: 'default'
        });
     }

     // Handle Nested Branches (Parallel/If)
     if (branches && Array.isArray(branches)) {
         let branchX = -200; // Start offset
         branches.forEach((branch: any, idx: number) => {
             // Branch with nested 'nodes'
             if (branch.nodes) {
                 branch.nodes.forEach((subNode: any, subIdx: number) => {
                     // Reset Y for start of branch?
                     // Simple layout logic: just cascade down
                     processNode(subNode, xOffset + branchX);
                     
                     // Connect Parallel Parent -> First Node of Branch
                     if (subIdx === 0) {
                          edges.push({
                             id: `e_${id}_${subNode.id}`,
                             source: id,
                             target: subNode.id,
                             type: 'default',
                             label: `branch ${idx + 1}`
                         });
                     }
                 });
             }
             branchX += 200;
         });
     }
  };

  if (data.nodes) {
      data.nodes.forEach((n: any) => processNode(n));
  }

  // Process explicit edges
  if (data.edges) {
      data.edges.forEach((e: any) => {
          edges.push({
              id: `e_${e.source}_${e.target}`,
              source: e.source,
              target: e.target,
              label: e.condition,
              data: { condition: e.condition },
              type: 'default'
          });
      });
  }

  return {
      nodes,
      edges,
      meta: data.workflow || { id: 'imported', name: 'Imported Flow' }
  };
};
