import React, { memo } from 'react';
import { Handle, Position, type NodeProps } from 'reactflow';
import type { SkriptNodeData } from '../../types';
import clsx from 'clsx';
import { Variable, Play, CheckCircle, Split, GitMerge, Repeat, Code } from 'lucide-react';

const icons: Record<string, React.ElementType> = {
  Start: Play,
  End: CheckCircle,
  Function: Code,
  Assign: Variable,
  If: Split,
  Parallel: GitMerge,
  Iteration: Repeat,
  Loop: Repeat,
};

const GenericNode = ({ data, selected }: NodeProps<SkriptNodeData>) => {
  const Icon = icons[data.kind] || Code;
  
  return (
    <div className={clsx(
      "px-4 py-2 shadow-md rounded-md bg-white border-2 min-w-[150px]",
      selected ? "border-blue-500 ring-2 ring-blue-200" : "border-gray-200 hover:border-gray-300"
    )}>
      {data.kind !== 'Start' && (
        <Handle type="target" position={Position.Top} className="!w-3 !h-3 !bg-gray-400" />
      )}
      
      <div className="flex items-center">
        <div className={clsx("rounded-full p-2 mr-2", {
          'bg-green-100 text-green-600': data.kind === 'Start',
          'bg-red-100 text-red-600': data.kind === 'End',
          'bg-blue-100 text-blue-600': data.kind === 'Function',
          'bg-purple-100 text-purple-600': data.kind === 'If',
          'bg-yellow-100 text-yellow-600': data.kind === 'Assign',
          'bg-orange-100 text-orange-600': data.kind === 'Parallel',
          'bg-indigo-100 text-indigo-600': data.kind === 'Iteration',
        })}>
          <Icon size={16} />
        </div>
        <div>
            <div className="text-sm font-bold text-gray-700">{data.label || data.kind}</div>
            {data.kind === 'Function' && data.functionName && (
                <div className="text-xs text-gray-500 mt-0.5 truncate max-w-[120px]">
                {data.functionName}
                </div>
            )}
            {data.kind === 'If' && (
                <div className="text-xs text-gray-500 mt-0.5">Conditional</div>
            )}
        </div>
      </div>
      

      {data.kind !== 'End' && (
        <Handle type="source" position={Position.Bottom} className="!w-3 !h-3 !bg-gray-400" />
      )}
    </div>
  );
};

export default memo(GenericNode);
