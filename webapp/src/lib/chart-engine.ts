import { init, use as registerCharts } from 'echarts/core';
import { BarChart, LineChart } from 'echarts/charts';
import { AriaComponent, GridComponent, TooltipComponent, DataZoomComponent } from 'echarts/components';
import { SVGRenderer } from 'echarts/renderers';

registerCharts([BarChart,LineChart,AriaComponent,GridComponent,TooltipComponent,DataZoomComponent,SVGRenderer]);
export { init };
