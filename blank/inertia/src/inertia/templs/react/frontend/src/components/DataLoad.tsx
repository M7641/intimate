import spinnerGif from '@/assets/spinner.gif';
import { useTheme } from '@/components/themeProvider.tsx';
import { cn } from '@/lib/utils.ts';

function DataLoadError(
    { error }: { error: Error | null }
) {
    if (!error) return null;

    return (
        <div className="error-message">
            <p>Error loading data: {error.message}</p>
        </div>
    );
}

function LoadingSpinner() {
  const { theme } = useTheme();

  let invertClass = '';
  if (theme === 'dark' || theme === 'cyberpunk' ) {
    invertClass = 'invert';
  }

  return (
    <div className="flex justify-center items-center min-h-[400px]">
      <img
        src={spinnerGif}
        className={cn("h-24 w-24", invertClass)}
      />
    </div>
  );
}




export default function DataLoading(
    { isPending, error, lightBackground }: { isPending: boolean; error?: Error | null; lightBackground?: boolean }
) {

    if (!isPending && !error) {
        return null;
    }

    let componentContent;
    if (isPending) {
      componentContent = <LoadingSpinner />;
    } else if (error) {
      componentContent = <DataLoadError error={error} />;
    }

    return (
      <div className="flex justify-center items-center mt-[16px]">
        <div className={
          lightBackground ?
          'h-[120px] w-[120px] bg-white p-4 rounded-full shadow-md flex justify-center items-center': ''
        }>
          {componentContent}
        </div>
      </div>
    );
}
