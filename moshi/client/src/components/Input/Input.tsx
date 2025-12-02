type InputProps = React.InputHTMLAttributes<HTMLInputElement> & {
  error?: string;
};

export const Input = ({ className, error, ...props }: InputProps) => {
  return (
    <div className="relative pb-8">
      <input
        {...props}
        className={`border-2 border-white bg-black p-2 p-2 text-white outline-none hover:bg-gray-800 focus:bg-gray-800 disabled:bg-gray-100 ${className ?? ""}`}
      />
      {error && <p className=" absolute text-red-800">{error}</p>}
    </div>
  );
};
