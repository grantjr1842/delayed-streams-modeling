import { createContext, useContext } from "react";
import type { WSMessage } from "../../protocol/types";
import type { ConnectionError } from "../../types/errors";

type SocketContextType = {
  isConnected: boolean;
  isConnecting: boolean;
  socket: WebSocket | null;
  sendMessage: (message: WSMessage) => void;
  error: ConnectionError | null;
  clearError: () => void;
  retryConnection: () => void;
  retryCount: number;
};

export const SocketContext = createContext<SocketContextType>({
  isConnected: false,
  isConnecting: false,
  socket: null,
  sendMessage: () => {},
  error: null,
  clearError: () => {},
  retryConnection: () => {},
  retryCount: 0,
});

export const useSocketContext = () => {
  return useContext(SocketContext);
};
