import { Button } from "../../../../components/Button/Button";
import {
  controlBOSMessage,
  controlEOSMessage,
} from "../../../../protocol/testMessages";
import { useSocketContext } from "../../SocketContext";

export const Controls = () => {
  const { sendMessage } = useSocketContext();

  const sendControlBOS = () => {
    sendMessage(controlBOSMessage);
  };

  const sendControlEOS = () => {
    sendMessage(controlEOSMessage);
  };
  return (
    <div className="flex w-full justify-between gap-3">
      <Button className="flex-grow" onClick={sendControlEOS}>
        eos
      </Button>
      <Button className="flex-grow" onClick={sendControlBOS}>
        bos
      </Button>
    </div>
  );
};
